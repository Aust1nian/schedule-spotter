//! Phase 5 fixture checks, run through `spotter_core::session::Session`, the
//! same calls the app makes: the saved workbook keeps formula-looking text
//! as data (`export_check`), saving somewhere unwritable fails cleanly
//! (`save_test`), and category N `steps` (answers, undoing a chip, column
//! roles, working days, edits, save and reopen) lead to `after_steps`.

use std::io::Read;
use std::path::PathBuf;

use serde_json::Value;
use spotter_core::export::{self, SaveError};
use spotter_core::session::{Session, Source};
use spotter_core::settings::Workdays;
use spotter_core::{Input, Report, Settings, Tiebreaker};

use super::{
    answers_from, assignment_json, compare_findings, diff, flag_json, info_json, question_matches,
    tuple,
};

pub fn session_for(input: &Input, settings: &Settings) -> Session {
    let source = match input {
        Input::File { name, bytes } => Source::File {
            name: name.clone(),
            bytes: bytes.clone(),
        },
        Input::Paste { text } => Source::Paste { text: text.clone() },
    };
    Session::new(source, settings.clone())
}

/// The worksheets of a saved workbook, as XML text.
fn sheet_xml(bytes: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else {
        return out;
    };
    for i in 0..zip.len() {
        let Ok(mut f) = zip.by_index(i) else { continue };
        let Ok(name) = f.name().map(|n| n.into_owned()) else {
            continue;
        };
        if name.starts_with("xl/worksheets/") {
            let mut text = String::new();
            let _ = f.read_to_string(&mut text);
            out.push((name, text));
        }
    }
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// CSV fields, honoring quotes.
fn csv_fields(text: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            (true, '"') => quoted = false,
            (true, c) => cur.push(c),
            (false, '"') if cur.is_empty() => quoted = true,
            (false, ',' | '\n') => fields.push(std::mem::take(&mut cur)),
            (false, '\r') => {}
            (false, c) => cur.push(c),
        }
    }
    if !cur.is_empty() {
        fields.push(cur);
    }
    fields
}

const RISKY: [char; 6] = ['=', '+', '-', '@', '\t', '\r'];

pub fn export_check(session: &Session, report: &Report, problem: &mut dyn FnMut(String)) {
    let bytes = export::workbook(session, report);
    let sheets = sheet_xml(&bytes);
    if sheets.is_empty() {
        problem("export: workbook has no worksheets".into());
    }
    for (name, xml) in &sheets {
        if xml.contains("<f>") || xml.contains("<f ") {
            problem(format!("export: {name} has a formula"));
        }
        if xml.contains("t=\"str\"") || xml.contains("t=\"e\"") {
            problem(format!("export: {name} has a computed cell type"));
        }
    }
    let all: String = sheets.iter().map(|(_, x)| x.as_str()).collect();
    for a in &report.reading.assignments {
        for text in [&a.person, &a.site, &a.note].into_iter().flatten() {
            if text.starts_with(RISKY) {
                let as_string = format!(">{}</t>", xml_escape(text));
                if !all.contains(&as_string) {
                    problem(format!("export: {text:?} is not written as a text cell"));
                }
            }
        }
    }
    let csv = String::from_utf8(export::findings_csv(report)).unwrap_or_default();
    for field in csv_fields(&csv) {
        if field.starts_with(RISKY) {
            problem(format!(
                "export: CSV field {field:?} could run as a formula"
            ));
        }
    }
}

pub fn save_test(
    spec: &Value,
    session: &Session,
    report: &Report,
    problem: &mut dyn FnMut(String),
) {
    if spec["target"].as_str() != Some("read-only") {
        problem(format!("save_test: unknown target {}", spec["target"]));
        return;
    }
    let expected = spec["error"].as_str().unwrap_or("save_failed");
    let dir = scratch_dir("read-only");
    if !make_read_only(&dir) {
        // A read-only folder can't be made here (Windows ignores the flag
        // on folders; root ignores permissions). The app's UI test covers it.
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let path = dir.join(export::default_name(report, &session.settings));
    let before = session.to_json();
    match export::save(&path, &export::workbook(session, report)) {
        Ok(()) => problem("save_test: saving to a read-only folder succeeded".into()),
        Err(SaveError::SaveFailed(why)) => {
            if expected != "save_failed" {
                problem(format!("save_test: expected {expected}, got save_failed"));
            }
            if why.trim().is_empty() {
                problem("save_test: no message for the owner".into());
            }
        }
    }
    let left: Vec<_> = std::fs::read_dir(&dir)
        .map(|d| d.flatten().map(|e| e.file_name()).collect())
        .unwrap_or_default();
    if !left.is_empty() {
        problem(format!("save_test: files left behind: {left:?}"));
    }
    if session.to_json() != before {
        problem("save_test: the check changed".into());
    }
    restore_writable(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "spotter-check-{}-{tag}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[cfg(unix)]
fn make_read_only(dir: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    if std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).is_err() {
        return false;
    }
    // Root writes anyway; then the test means nothing.
    let probe = dir.join(".probe");
    if std::fs::write(&probe, b"x").is_ok() {
        let _ = std::fs::remove_file(probe);
        return false;
    }
    true
}

#[cfg(not(unix))]
fn make_read_only(_dir: &std::path::Path) -> bool {
    false
}

#[cfg(unix)]
fn restore_writable(dir: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn restore_writable(_dir: &std::path::Path) {}

fn workdays_from(v: &Value) -> Option<Workdays> {
    let names = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
    let mut w = [false; 7];
    for d in v.as_array()? {
        let i = names.iter().position(|n| Some(*n) == d.as_str())?;
        w[i] = true;
    }
    Some(Workdays(w))
}

/// Runs category N steps on `session`, as the owner would in the app.
pub fn run_steps(
    steps: &[Value],
    session: &mut Session,
    tiebreak: &dyn Tiebreaker,
    problem: &mut dyn FnMut(String),
) {
    for step in steps {
        let report = session.report(tiebreak);
        let Some((op, arg)) = step.as_object().and_then(|o| o.iter().next()) else {
            problem(format!("step: not an object: {step}"));
            continue;
        };
        match op.as_str() {
            "answer" => {
                let answers = answers_from(std::slice::from_ref(arg), &report.questions);
                session.answers.items.extend(answers.items);
            }
            "undo_merge" => {
                let (from, into) = (arg["from"].as_str(), arg["into"].as_str());
                match report
                    .merges
                    .iter()
                    .find(|m| Some(m.from.as_str()) == from && Some(m.into.as_str()) == into)
                {
                    Some(m) => session.undo_merge(&m.clone()),
                    None => problem(format!(
                        "step undo_merge: no chip {from:?} → {into:?} (chips: {:?})",
                        report
                            .merges
                            .iter()
                            .map(|m| format!("{} → {}", m.from, m.into))
                            .collect::<Vec<_>>()
                    )),
                }
            }
            "column_role" => {
                let header = arg["header"].as_str().unwrap_or("");
                if !report.reading.columns.iter().any(|c| c.header == header) {
                    problem(format!(
                        "step column_role: no column {header:?} in What I read ({:?})",
                        report
                            .reading
                            .columns
                            .iter()
                            .map(|c| &c.header)
                            .collect::<Vec<_>>()
                    ));
                }
                session.set_column_role(header, arg["role"].as_str().unwrap_or(""));
            }
            "layout" => session.set_layout(arg.as_str().unwrap_or("")),
            "workdays" => match workdays_from(arg) {
                Some(w) => session.set_workdays(w),
                None => problem(format!("step workdays: bad days {arg}")),
            },
            "day_off" => match arg.as_str().and_then(|d| d.parse().ok()) {
                Some(d) => session.toggle_day_off(d),
                None => problem(format!("step day_off: bad date {arg}")),
            },
            "edit" => {
                let Some(mut table) = session.table(&report.reading.sheet) else {
                    problem("step edit: the table can't be edited".into());
                    continue;
                };
                let (r, c) = (
                    arg["row"].as_u64().unwrap_or(0) as usize,
                    arg["col"].as_u64().unwrap_or(0) as usize,
                );
                if let Some(text) = arg.get("text").and_then(Value::as_str) {
                    while table.len() <= r {
                        table.push(Vec::new());
                    }
                    while table[r].len() <= c {
                        table[r].push(String::new());
                    }
                    table[r][c] = text.to_string();
                } else if arg.get("delete_row").and_then(Value::as_bool) == Some(true)
                    && r < table.len()
                {
                    table.remove(r);
                }
                session.set_table(&table);
            }
            "save_reopen" => {
                let before = report.flags.clone();
                let base = scratch_dir("save");
                let path = base.join(arg.as_str().unwrap_or("check.xlsx"));
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match export::save(&path, &export::workbook(session, &report)) {
                    Err(e) => problem(format!("step save_reopen: save failed: {e}")),
                    Ok(()) => match std::fs::read(&path).ok().and_then(|b| export::restore(&b)) {
                        None => problem(format!(
                            "step save_reopen: {} didn't reopen as a saved check",
                            path.display()
                        )),
                        Some(back) => {
                            *session = back;
                            let after = session.report(tiebreak);
                            if after.flags != before {
                                problem("step save_reopen: findings changed on reopen".into());
                            }
                        }
                    },
                }
                let _ = std::fs::remove_dir_all(&base);
            }
            other => problem(format!("step: unknown op {other:?}")),
        }
    }
}

/// `after_steps`: the same keys as the top level, checked on the session.
pub fn check_after(target: &Value, report: &Report, problem: &mut dyn FnMut(String)) {
    if let Some(exp) = target.get("assignments").and_then(Value::as_array) {
        let e: Vec<String> = exp.iter().map(tuple).collect();
        let a: Vec<String> = report
            .reading
            .assignments
            .iter()
            .map(|x| tuple(&assignment_json(x)))
            .collect();
        let (missing, extra) = diff(&e, &a);
        for m in &missing {
            problem(format!("after steps, missing: {m}"));
        }
        if target
            .get("assignments_exact")
            .and_then(Value::as_bool)
            .unwrap_or(true)
        {
            for x in &extra {
                problem(format!("after steps, extra:   {x}"));
            }
        }
    }
    if let Some(exp) = target.get("flags").and_then(Value::as_array) {
        let exact = target
            .get("flags_exact")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let actual: Vec<Value> = report.flags.iter().map(flag_json).collect();
        compare_findings("after steps, flag", exp, &actual, exact, problem);
    }
    if let Some(exp) = target.get("info").and_then(Value::as_array) {
        let exact = target
            .get("info_exact")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let actual: Vec<Value> = report.info.iter().map(info_json).collect();
        compare_findings("after steps, info", exp, &actual, exact, problem);
    }
    let questions: Vec<Value> = report
        .questions
        .iter()
        .map(|q| serde_json::to_value(q).expect("json"))
        .collect();
    for spec in target
        .get("ask")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if !questions.iter().any(|q| question_matches(spec, q)) {
            problem(format!("after steps, not asked: {spec}"));
        }
    }
    for spec in target
        .get("must_not_ask")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(q) = questions.iter().find(|q| question_matches(spec, q)) {
            problem(format!("after steps, asked but must not: {q}"));
        }
    }
    for col in target
        .get("columns")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (h, r) = (col["header"].as_str(), col["role"].as_str());
        if !report
            .reading
            .columns
            .iter()
            .any(|c| Some(c.header.as_str()) == h && Some(c.role.as_str()) == r)
        {
            problem(format!(
                "after steps, column {h:?} is not {r:?} ({:?})",
                report
                    .reading
                    .columns
                    .iter()
                    .map(|c| format!("{}={}", c.header, c.role))
                    .collect::<Vec<_>>()
            ));
        }
    }
    if let Some(chips) = target.get("merges").and_then(Value::as_array) {
        let actual: Vec<String> = report
            .merges
            .iter()
            .map(|m| format!("{} → {}", m.from, m.into))
            .collect();
        let expected: Vec<String> = chips
            .iter()
            .map(|c| {
                format!(
                    "{} → {}",
                    c["from"].as_str().unwrap_or(""),
                    c["into"].as_str().unwrap_or("")
                )
            })
            .collect();
        let (missing, extra) = diff(&expected, &actual);
        for m in missing {
            problem(format!("after steps, chip missing: {m}"));
        }
        for x in extra {
            problem(format!("after steps, extra chip: {x}"));
        }
    }
}
