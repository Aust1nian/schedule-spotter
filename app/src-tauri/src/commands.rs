//! What the page can ask of the app. The page never touches files: opening
//! and saving go through the system dialogs here, and the page only gets
//! results back (DECISIONS.md, "File dialogs through rfd").

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use chrono::NaiveDate;
use serde::Serialize;
use spotter_core::session::{Session, Source};
use spotter_core::settings::Workdays;
use spotter_core::{Report, Settings, export};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

use crate::{model, selftest, ui};

pub struct AppState {
    pub started: Instant,
    pub self_test: Option<selftest::SelfTest>,
    pub scratch: PathBuf,
    pub session: Mutex<Option<Session>>,
    pub report: Mutex<Report>,
    pub generation: AtomicU64,
    /// What happened, for diagnostics: no schedule content.
    pub log: Mutex<Vec<String>>,
}

impl AppState {
    pub fn new(started: Instant, self_test: Option<selftest::SelfTest>, scratch: PathBuf) -> Self {
        AppState {
            started,
            self_test,
            scratch,
            session: Mutex::new(None),
            report: Mutex::new(Report::default()),
            generation: AtomicU64::new(0),
            log: Mutex::new(Vec::new()),
        }
    }

    pub fn note(&self, line: impl Into<String>) {
        let at = self.started.elapsed().as_secs_f64();
        if let Ok(mut log) = self.log.lock() {
            log.push(format!("[{at:7.1} s] {}", line.into()));
        }
    }
}

type Result<T> = std::result::Result<T, String>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Recomputes with the rules (fast, returned now), then, if the model is
/// ready, again with the model on a worker thread, sent as a "report" event.
pub fn refresh(app: &AppHandle) -> ui::UiReport {
    let state = app.state::<AppState>();
    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let Some(session) = lock(&state.session).clone() else {
        return ui::build(
            &Session::example(Settings::default()),
            &Report::default(),
            generation,
            "off",
        );
    };
    let started = Instant::now();
    let report = session.report_model_off();
    state.note(format!(
        "rules: {} findings, {} questions in {} ms",
        report.flags.len(),
        report.questions.len(),
        started.elapsed().as_millis()
    ));
    let smart = if model::ready() { "pending" } else { "off" };
    let out = ui::build(&session, &report, generation, smart);
    *lock(&state.report) = report;
    if model::ready() {
        let app = app.clone();
        std::thread::spawn(move || {
            let state = app.state::<AppState>();
            let started = Instant::now();
            let report = session.report(model::tiebreak());
            if state.generation.load(Ordering::SeqCst) != generation {
                return; // the owner changed something meanwhile
            }
            state.note(format!(
                "model: {} findings, {} questions in {} ms",
                report.flags.len(),
                report.questions.len(),
                started.elapsed().as_millis()
            ));
            let out = ui::build(&session, &report, generation, "done");
            *lock(&state.report) = report;
            let _ = app.emit("report", out);
        });
    }
    out
}

fn change(app: &AppHandle, f: impl FnOnce(&mut Session)) -> ui::UiReport {
    {
        let state = app.state::<AppState>();
        let mut session = lock(&state.session);
        if let Some(s) = session.as_mut() {
            f(s);
        }
    }
    refresh(app)
}

fn today(state: &AppState, page_today: &str) -> NaiveDate {
    // The self-test pins the date so the page and the core agree.
    if state.self_test.is_some() {
        return Settings::default().today;
    }
    page_today
        .parse()
        .unwrap_or_else(|_| Settings::default().today)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mode {
    self_test: bool,
    ui_test: bool,
    version: &'static str,
}

#[tauri::command]
pub fn app_mode(state: State<'_, AppState>) -> Mode {
    if let Some(test) = &state.self_test {
        test.mark_page_ready(state.started);
    }
    Mode {
        self_test: state.self_test.is_some(),
        ui_test: cfg!(feature = "ui-test"),
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// The first report: the example week, unless a check is already open
/// (the page reloaded).
#[tauri::command]
pub async fn start(app: AppHandle, today_iso: String) -> ui::UiReport {
    {
        let state = app.state::<AppState>();
        let mut session = lock(&state.session);
        if session.is_none() {
            let settings = Settings {
                today: today(&state, &today_iso),
                ..Settings::default()
            };
            *session = Some(Session::example(settings));
        }
    }
    refresh(&app)
}

#[tauri::command]
pub async fn example(app: AppHandle, today_iso: String) -> ui::UiReport {
    {
        let state = app.state::<AppState>();
        let settings = Settings {
            today: today(&state, &today_iso),
            ..Settings::default()
        };
        *lock(&state.session) = Some(Session::example(settings));
        state.note("opened the example");
    }
    refresh(&app)
}

#[tauri::command]
pub async fn answer(app: AppHandle, subject: String, option: String) -> Result<ui::UiReport> {
    let question = {
        let state = app.state::<AppState>();
        let report = lock(&state.report);
        report
            .questions
            .iter()
            .find(|q| q.subject() == subject)
            .cloned()
            .ok_or("That question was already settled.")?
    };
    let view = spotter_core::describe::question(&question);
    let answer = view.answer_for(&option);
    Ok(change(&app, |s| s.answer(question, answer)))
}

#[tauri::command]
pub async fn undo_chip(app: AppHandle, index: usize) -> Result<ui::UiReport> {
    let merge = {
        let state = app.state::<AppState>();
        let report = lock(&state.report);
        report
            .merges
            .get(index)
            .cloned()
            .ok_or("That match is gone.")?
    };
    Ok(change(&app, |s| s.undo_merge(&merge)))
}

/// Takes back an answer: the question goes back to its default reading.
#[tauri::command]
pub async fn forget_answer(app: AppHandle, subject: String) -> ui::UiReport {
    change(&app, |s| {
        s.answers.items.retain(|(q, _)| q.subject() != subject);
    })
}

#[tauri::command]
pub async fn set_role(app: AppHandle, header: String, role: String) -> ui::UiReport {
    change(&app, |s| s.set_column_role(&header, &role))
}

#[tauri::command]
pub async fn set_layout(app: AppHandle, layout: String) -> ui::UiReport {
    change(&app, |s| s.set_layout(&layout))
}

#[tauri::command]
pub async fn set_sheet(app: AppHandle, sheet: String) -> ui::UiReport {
    change(&app, |s| s.set_sheet(&sheet))
}

#[tauri::command]
pub async fn set_workdays(app: AppHandle, days: [bool; 7]) -> ui::UiReport {
    change(&app, |s| s.set_workdays(Workdays(days)))
}

#[tauri::command]
pub async fn toggle_day_off(app: AppHandle, date: String) -> Result<ui::UiReport> {
    let date: NaiveDate = date.parse().map_err(|_| "bad date")?;
    Ok(change(&app, |s| s.toggle_day_off(date)))
}

#[derive(Serialize)]
pub struct Table {
    rows: Option<Vec<Vec<String>>>,
    why: Option<String>,
}

#[tauri::command]
pub async fn table(app: AppHandle) -> Table {
    let state = app.state::<AppState>();
    let session = lock(&state.session).clone();
    let sheet = lock(&state.report).reading.sheet.clone();
    match session.and_then(|s| s.table(&sheet)) {
        Some(rows) => Table {
            rows: Some(rows),
            why: None,
        },
        None => Table {
            rows: None,
            why: Some(format!(
                "This schedule is too big to edit here (over {} rows or {} columns). Edit it in your spreadsheet and open it again.",
                spotter_core::session::EDIT_MAX_ROWS,
                spotter_core::session::EDIT_MAX_COLS
            )),
        },
    }
}

#[tauri::command]
pub async fn set_table(app: AppHandle, rows: Vec<Vec<String>>) -> ui::UiReport {
    change(&app, |s| s.set_table(&rows))
}

#[tauri::command]
pub async fn paste(app: AppHandle, text: String) -> Result<ui::UiReport> {
    if text.trim().is_empty() {
        return Err("There was nothing to paste.".into());
    }
    {
        let state = app.state::<AppState>();
        let settings = settings_for_new(&state);
        state.note(format!("pasted {} lines", text.lines().count()));
        *lock(&state.session) = Some(Session::new(Source::Paste { text }, settings));
    }
    Ok(refresh(&app))
}

/// A new schedule keeps the owner's working days and today's date.
fn settings_for_new(state: &AppState) -> Settings {
    let current = lock(&state.session)
        .as_ref()
        .map(|s| s.settings.clone())
        .unwrap_or_default();
    Settings {
        days_off: Default::default(),
        ..current
    }
}

/// Opens a file the owner chose or dropped: a saved check reopens with every
/// answer; anything else is a new schedule.
pub fn open_path(app: &AppHandle, path: &Path) -> Result<ui::UiReport> {
    let started = Instant::now();
    let bytes = std::fs::read(path).map_err(|e| format!("I couldn't open that file ({e})."))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Schedule".into());
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    {
        let state = app.state::<AppState>();
        let session = match export::restore(&bytes) {
            Some(saved) => {
                state.note(format!("reopened a saved check (.{ext})"));
                saved
            }
            None => {
                state.note(format!(
                    "opened .{ext} ({} bytes) in {} ms",
                    bytes.len(),
                    started.elapsed().as_millis()
                ));
                Session::new(Source::File { name, bytes }, settings_for_new(&state))
            }
        };
        *lock(&state.session) = Some(session);
    }
    Ok(refresh(app))
}

const OPEN_EXTENSIONS: [&str; 10] = [
    "csv", "tsv", "txt", "xlsx", "xls", "xlsm", "xlsb", "ods", "numbers", "docx",
];

/// `Ok(None)`: the owner cancelled.
#[tauri::command]
pub async fn open_file(app: AppHandle, window: WebviewWindow) -> Result<Option<ui::UiReport>> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title("Open a schedule")
        .add_filter("Schedules", &OPEN_EXTENSIONS)
        .add_filter("All files", &["*"])
        .set_parent(&window)
        .pick_file()
        .await;
    match picked {
        Some(file) => open_path(&app, file.path()).map(Some),
        None => Ok(None),
    }
}

#[derive(Serialize)]
pub struct Saved {
    name: String,
    format: &'static str,
}

/// Writes the check to `path`: the workbook, or the findings CSV when the
/// name ends in .csv.
pub fn save_to(app: &AppHandle, path: &Path) -> Result<Saved> {
    let state = app.state::<AppState>();
    let session = lock(&state.session).clone().ok_or("Nothing to save yet.")?;
    let report = lock(&state.report).clone();
    let csv = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    let path = if path.extension().is_none() {
        path.with_extension("xlsx")
    } else {
        path.to_path_buf()
    };
    let bytes = if csv {
        export::findings_csv(&report)
    } else {
        export::workbook(&session, &report)
    };
    export::save(&path, &bytes).map_err(|e| e.to_string())?;
    state.note(format!(
        "saved {} ({} bytes)",
        if csv { "findings CSV" } else { "workbook" },
        bytes.len()
    ));
    Ok(Saved {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        format: if csv { "csv" } else { "xlsx" },
    })
}

/// `Ok(None)`: the owner cancelled.
#[tauri::command]
pub async fn save(app: AppHandle, window: WebviewWindow) -> Result<Option<Saved>> {
    let default_name = {
        let state = app.state::<AppState>();
        let session = lock(&state.session).clone();
        let report = lock(&state.report);
        export::default_name(&report, &session.map(|s| s.settings).unwrap_or_default())
    };
    let picked = rfd::AsyncFileDialog::new()
        .set_title("Save the schedule check")
        .set_file_name(&default_name)
        .add_filter("Excel workbook (reopens with your answers)", &["xlsx"])
        .add_filter("CSV, findings only", &["csv"])
        .set_parent(&window)
        .save_file()
        .await;
    match picked {
        Some(file) => save_to(&app, file.path()).map(Some),
        None => Ok(None),
    }
}

fn app_lines(state: &AppState) -> Vec<String> {
    let m = model::status();
    let mut lines = vec![
        format!("Version {}", env!("CARGO_PKG_VERSION")),
        format!(
            "System: {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
        format!(
            "Model: variant {}, {:?}{}{}{}",
            model::VARIANT,
            m.state,
            if m.detail.is_empty() {
                String::new()
            } else {
                format!(" ({})", m.detail)
            },
            m.load_secs
                .map(|s| format!(", loaded in {s:.1} s"))
                .unwrap_or_default(),
            m.backend.map(|b| format!(" on {b}")).unwrap_or_default(),
        ),
        format!(
            "Peak memory: {:.0} MB",
            crate::metrics::peak_rss_bytes() as f64 / 1_048_576.0
        ),
        String::new(),
        "Log:".into(),
    ];
    lines.extend(lock(&state.log).iter().cloned());
    lines
}

/// The diagnostics text, shown to the owner before they save it.
#[tauri::command]
pub async fn diagnostics(app: AppHandle) -> String {
    let state = app.state::<AppState>();
    let session = lock(&state.session)
        .clone()
        .unwrap_or_else(|| Session::example(Settings::default()));
    let report = lock(&state.report).clone();
    spotter_core::diagnostics::text(&session, &report, &app_lines(&state))
}

#[tauri::command]
pub async fn save_diagnostics(window: WebviewWindow, text: String) -> Result<Option<String>> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title("Save diagnostics")
        .set_file_name("Schedule Spotter diagnostics.txt")
        .add_filter("Text", &["txt"])
        .set_parent(&window)
        .save_file()
        .await;
    match picked {
        Some(file) => {
            export::save(file.path(), text.as_bytes()).map_err(|e| e.to_string())?;
            Ok(Some(file.file_name()))
        }
        None => Ok(None),
    }
}

#[derive(Serialize)]
pub struct About {
    version: &'static str,
    variant: &'static str,
    model_name: &'static str,
    model_license: &'static str,
    model_status: String,
    built_with_llama: bool,
}

#[tauri::command]
pub fn about() -> About {
    let m = model::status();
    About {
        version: env!("CARGO_PKG_VERSION"),
        variant: model::VARIANT,
        model_name: model::NAME,
        model_license: model::LICENSE,
        model_status: match m.state {
            model::State::Ready => format!(
                "Smart matching ready (loaded in {:.1} s)",
                m.load_secs.unwrap_or(0.0)
            ),
            model::State::Loading => "Smart matching is starting".into(),
            model::State::Off if m.detail.is_empty() => "Smart matching off".into(),
            model::State::Off => format!("Smart matching off: {}", m.detail),
        },
        built_with_llama: model::VARIANT == "C",
    }
}

/// A script error on the page: logged for diagnostics; fails a self-test.
#[tauri::command]
pub fn page_error(state: State<'_, AppState>, message: String) {
    state.note(format!("page error: {message}"));
    if let Some(test) = &state.self_test {
        test.fail_now(&format!("page error: {message}"));
    }
}

#[tauri::command]
pub fn self_test_done(app: AppHandle, state: State<'_, AppState>, rendered: selftest::Rendered) {
    if let Some(test) = &state.self_test {
        let code = test.finish(state.started, &rendered);
        let linger = test.linger();
        // Stay open a while so CI can watch for late network activity.
        std::thread::spawn(move || {
            std::thread::sleep(linger);
            app.exit(code);
        });
    }
}

/// UI tests only (built with `--features ui-test`): open and save without
/// the system dialogs, which WebDriver can't drive. Never in a shipped app.
#[cfg(feature = "ui-test")]
#[tauri::command]
pub async fn test_open(app: AppHandle, path: String) -> Result<ui::UiReport> {
    // As if dropped on the window: the page draws it from the event.
    let report = open_path(&app, Path::new(&path))?;
    let _ = app.emit("report", report.clone());
    Ok(report)
}

#[cfg(feature = "ui-test")]
#[tauri::command]
pub async fn test_save(app: AppHandle, path: String) -> Result<Saved> {
    save_to(&app, Path::new(&path))
}
