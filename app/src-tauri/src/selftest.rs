//! `--self-test`: opens the real window, renders the built-in example, checks
//! the page shows what the core found, and, in a build with a model, loads
//! the bundled model from where it was installed and asks it 20 tiebreaker
//! questions. Prints JSON metrics and exits nonzero if any threshold is
//! missed. CI runs it on every runner.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use spotter_core::Settings;
use spotter_core::matching::Verdict;
use spotter_core::session::Session;

use crate::{metrics, model};

/// Give up and fail if the page hasn't reported back by then.
const WATCHDOG: Duration = Duration::from_secs(90);
/// How long a build with a model waits for it to load (the benchmark's
/// slowest runner takes about 21 s; this only catches a hang).
const MODEL_WAIT: Duration = Duration::from_secs(240);
/// Whole-app ceiling with the model not loaded (spec: Hard constraints).
const MAX_RSS_MODEL_OFF_MB: f64 = 300.0;
/// Whole-app hard ceiling with the model loaded.
const MAX_RSS_MB: f64 = 4096.0;
/// The tiebreaker questions: the first of each task from the benchmark set,
/// 20 in all.
const QUESTIONS: [(&str, &str, usize); 5] = [
    (
        "t1",
        include_str!("../../../bench/questions/t1_same_person.jsonl"),
        5,
    ),
    (
        "t2",
        include_str!("../../../bench/questions/t2_same_site.jsonl"),
        5,
    ),
    (
        "t3",
        include_str!("../../../bench/questions/t3_cell_meaning.jsonl"),
        4,
    ),
    (
        "t4",
        include_str!("../../../bench/questions/t4_column_role.jsonl"),
        3,
    ),
    (
        "t5",
        include_str!("../../../bench/questions/t5_free_text.jsonl"),
        3,
    ),
];
/// The window must show its first page within this long.
const MAX_PAGE_READY_MS: u128 = 15_000;

pub struct Args {
    pub self_test: bool,
    pub json_out: Option<PathBuf>,
    /// Seconds to stay open after reporting, so CI can watch for late
    /// network activity.
    pub linger: Duration,
    /// `--install-model <setup.exe>` (the Windows installer's hook).
    pub install_model: Option<PathBuf>,
}

impl Args {
    pub fn parse(mut args: impl Iterator<Item = String>) -> Args {
        let mut parsed = Args {
            self_test: false,
            json_out: None,
            linger: Duration::ZERO,
            install_model: None,
        };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--self-test" => parsed.self_test = true,
                "--json-out" => parsed.json_out = args.next().map(PathBuf::from),
                "--install-model" => parsed.install_model = args.next().map(PathBuf::from),
                "--linger" => {
                    let secs = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                    parsed.linger = Duration::from_secs(secs);
                }
                _ => {}
            }
        }
        parsed
    }
}

/// What the page drew, counted from the DOM.
#[derive(Debug, serde::Deserialize)]
pub struct Rendered {
    pub flags: usize,
    pub questions: usize,
    pub people: usize,
}

pub struct SelfTest {
    json_out: Option<PathBuf>,
    linger: Duration,
    page_ready_ms: OnceLock<u128>,
    done: Arc<AtomicBool>,
}

impl SelfTest {
    pub fn start(json_out: Option<PathBuf>, linger: Duration) -> SelfTest {
        let done = Arc::new(AtomicBool::new(false));
        let watchdog_done = Arc::clone(&done);
        let watchdog_out = json_out.clone();
        let watchdog = if model::VARIANT == "off" {
            WATCHDOG
        } else {
            WATCHDOG + MODEL_WAIT + Duration::from_secs(20 * 6)
        };
        std::thread::spawn(move || {
            std::thread::sleep(watchdog);
            if !watchdog_done.swap(true, Ordering::SeqCst) {
                let report = json!({
                    "self_test": "fail",
                    "failures": [format!("no result within {} s", watchdog.as_secs())],
                });
                emit(&report, watchdog_out.as_ref());
                std::process::exit(3);
            }
        });
        SelfTest {
            json_out,
            linger,
            page_ready_ms: OnceLock::new(),
            done,
        }
    }

    /// Fails at once (a script error on the page) instead of waiting for the
    /// watchdog.
    pub fn fail_now(&self, why: &str) {
        if self.done.swap(true, Ordering::SeqCst) {
            return;
        }
        let report = json!({"self_test": "fail", "failures": [why]});
        emit(&report, self.json_out.as_ref());
        std::process::exit(1);
    }

    pub fn linger(&self) -> Duration {
        self.linger
    }

    pub fn mark_page_ready(&self, started: Instant) {
        let _ = self.page_ready_ms.set(started.elapsed().as_millis());
    }

    /// Checks thresholds, prints the metrics, and returns the exit code.
    /// In a build with a model this waits for the model, so it runs off the
    /// main thread.
    pub fn finish(&self, started: Instant, rendered: &Rendered) -> i32 {
        if self.done.load(Ordering::SeqCst) {
            return 3;
        }
        let model_part = (model::VARIANT != "off").then(model_check);
        if self.done.swap(true, Ordering::SeqCst) {
            return 3;
        }
        let example = Session::example(Settings::default()).report_model_off();
        let core = Rendered {
            flags: example.flags.len(),
            questions: example.questions.len(),
            people: example.week.people.len(),
        };
        let page_ready_ms = self.page_ready_ms.get().copied();
        let peak_rss_mb = metrics::peak_rss_bytes() as f64 / (1024.0 * 1024.0);

        let mut failures = Vec::new();
        match &model_part {
            None if peak_rss_mb > MAX_RSS_MODEL_OFF_MB => failures.push(format!(
                "peak RSS {peak_rss_mb:.0} MB > {MAX_RSS_MODEL_OFF_MB} MB with model off"
            )),
            Some(_) if peak_rss_mb > MAX_RSS_MB => failures.push(format!(
                "peak RSS {peak_rss_mb:.0} MB > {MAX_RSS_MB} MB with the model loaded"
            )),
            _ => {}
        }
        if let Some((_, model_failures)) = &model_part {
            failures.extend(model_failures.iter().cloned());
        }
        match page_ready_ms {
            Some(ms) if ms <= MAX_PAGE_READY_MS => {}
            Some(ms) => failures.push(format!("page ready after {ms} ms > {MAX_PAGE_READY_MS} ms")),
            None => failures.push("page never asked for its mode".into()),
        }
        if core.flags == 0 || core.questions == 0 {
            failures.push("the example shows no findings or no questions".into());
        }
        for (what, page, want) in [
            ("findings", rendered.flags, core.flags),
            ("questions", rendered.questions, core.questions),
            ("week-grid rows", rendered.people, core.people),
        ] {
            if page != want {
                failures.push(format!("page shows {page} {what}, core found {want}"));
            }
        }

        let mut report = json!({
            "self_test": if failures.is_empty() { "pass" } else { "fail" },
            "version": env!("CARGO_PKG_VERSION"),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "model": crate::model::VARIANT,
            "page_ready_ms": page_ready_ms,
            "total_ms": started.elapsed().as_millis(),
            "peak_rss_mb": (peak_rss_mb * 10.0).round() / 10.0,
            "flags_core": core.flags,
            "flags_rendered": rendered.flags,
            "questions_core": core.questions,
            "questions_rendered": rendered.questions,
            "people_core": core.people,
            "people_rendered": rendered.people,
            "failures": failures,
        });
        if let Some((metrics, _)) = model_part {
            report["model_check"] = metrics;
        }
        emit(&report, self.json_out.as_ref());
        if report["self_test"] == "pass" { 0 } else { 1 }
    }
}

/// Waits for the model, then asks it the 20 questions. Gated: the model
/// loads (which includes its SHA-256 check) and no llama.cpp call fails.
/// Reported, not gated: load time, latency and how many it answered, which
/// depend on the runner and are the benchmark's job (DECISIONS.md, Phase 6).
fn model_check() -> (Value, Vec<String>) {
    let waited = Instant::now();
    while model::status().state == model::State::Loading && waited.elapsed() < MODEL_WAIT {
        std::thread::sleep(Duration::from_millis(200));
    }
    let status = model::status();
    let mut out = json!({
        "file": model::FILE,
        "state": status.state,
        "detail": status.detail,
        "load_s": status.load_secs,
        "backend": status.backend,
    });
    match status.state {
        model::State::Ready => {}
        model::State::Loading => {
            let why = format!("model still loading after {} s", MODEL_WAIT.as_secs());
            return (out, vec![why]);
        }
        model::State::Off => {
            let why = format!("model did not load: {}", status.detail);
            return (out, vec![why]);
        }
    }

    let tb = model::tiebreak();
    let (mut asked, mut answered, mut right) = (0, 0, 0);
    let mut latencies = Vec::new();
    for (task, lines, n) in QUESTIONS {
        for q in lines
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .take(n)
        {
            let s = |k: &str| q[k].as_str().unwrap_or_default().to_string();
            let list = |k: &str| -> Vec<String> {
                q[k].as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            };
            let t = Instant::now();
            let said: Option<String> = match task {
                "t1" | "t2" => {
                    let v = if task == "t1" {
                        tb.same_person(&s("a"), &s("b"), &list("roster"))
                    } else {
                        tb.same_site(&s("a"), &s("b"), &list("sites"))
                    };
                    match v {
                        Verdict::Same => Some("same".into()),
                        Verdict::Different => Some("different".into()),
                        Verdict::Unsure => None,
                    }
                }
                "t3" => tb.cell_meaning(&s("text"), &s("column_role"), &list("neighbors")),
                "t4" => tb.column_role(&s("header"), &list("samples")),
                _ => tb
                    .free_text(&s("line"), &list("roster"), &list("sites"))
                    .map(|e| format!("{} entries", e.len())),
            };
            latencies.push(t.elapsed().as_secs_f64());
            asked += 1;
            if let Some(said) = said {
                answered += 1;
                if q["label"].as_str() == Some(said.as_str()) {
                    right += 1;
                }
            }
        }
    }
    latencies.sort_by(f64::total_cmp);
    let pct = |p: f64| {
        let i = ((latencies.len().max(1) - 1) as f64 * p) as usize;
        latencies.get(i).map(|s| (s * 100.0).round() / 100.0)
    };
    out["questions"] = json!(asked);
    out["answered"] = json!(answered);
    out["right_of_closed"] = json!(right);
    out["latency_p50_s"] = json!(pct(0.5));
    out["latency_p95_s"] = json!(pct(0.95));
    let mut failures = Vec::new();
    if asked != 20 {
        failures.push(format!("asked {asked} tiebreaker questions, not 20"));
    }
    if model::failures() > 0 {
        failures.push(format!("{} model calls failed", model::failures()));
    }
    (out, failures)
}

fn emit(report: &Value, json_out: Option<&PathBuf>) {
    let line = report.to_string();
    println!("{line}");
    if let Some(path) = json_out
        && let Err(err) = std::fs::write(path, format!("{line}\n"))
    {
        eprintln!("could not write {}: {err}", path.display());
    }
}

/// A Windows release build is a GUI program with no console; reattach to the
/// terminal that launched it so the JSON shows up there.
#[cfg(windows)]
pub fn attach_parent_console() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    // SAFETY: plain Win32 call; failure (no parent console) is harmless.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
pub fn attach_parent_console() {}
