//! Runs the pipeline on fixtures and generated cases and compares against
//! `expected.json` (see fixtures/FORMAT.md).
//!
//!   spotter-check run <file> [--paste] [--today YYYY-MM-DD]
//!   spotter-check fixtures <dir>... [--only ID] [--verbose] [--json out.json]
//!   spotter-check generated <dir> [--verbose] [--min-recall 0.99]

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::NaiveDate;
use serde_json::{Value, json};
use spotter_core::model::{Assignment, Status};
use spotter_core::settings::Workdays;
use spotter_core::times::fmt;
use spotter_core::{Input, Report, Settings, analyze};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("run") => cmd_run(&args[1..]),
        Some("fixtures") => cmd_fixtures(&args[1..]),
        Some("generated") => cmd_generated(&args[1..]),
        _ => {
            eprintln!("usage: spotter-check run <file> | fixtures <dir>... | generated <dir>");
            2
        }
    };
    std::process::exit(code);
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn cmd_run(args: &[String]) -> i32 {
    let Some(path) = args.first() else { return 2 };
    let mut settings = Settings::default();
    if let Some(t) = flag(args, "--today") {
        settings.today = NaiveDate::parse_from_str(&t, "%Y-%m-%d").expect("--today YYYY-MM-DD");
    }
    let input = load_input(Path::new(path), args.iter().any(|a| a == "--paste"));
    let report = analyze(&input, &settings);
    println!(
        "{}",
        serde_json::to_string_pretty(&report_json(&report)).expect("json")
    );
    0
}

fn load_input(path: &Path, paste: bool) -> Input {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    if paste {
        Input::Paste {
            text: String::from_utf8_lossy(&bytes).into_owned(),
        }
    } else {
        Input::File {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            bytes,
        }
    }
}

fn report_json(report: &Report) -> Value {
    let assignments: Vec<Value> = report
        .reading
        .assignments
        .iter()
        .map(assignment_json)
        .collect();
    json!({
        "error": report.error,
        "sheet": report.reading.sheet,
        "layout": report.reading.layout,
        "arrangement": report.reading.arrangement,
        "columns": report.reading.columns,
        "window": report.reading.window,
        "grid_jobs": report.reading.grid_jobs,
        "assignments": assignments,
        "questions": report.reading.questions,
        "notes": report.reading.notes,
    })
}

fn status_name(s: Status) -> &'static str {
    match s {
        Status::Off => "off",
        Status::Pto => "pto",
        Status::Vacation => "vacation",
        Status::Sick => "sick",
        Status::Holiday => "holiday",
        Status::Out => "out",
    }
}

fn assignment_json(a: &Assignment) -> Value {
    let mut m = serde_json::Map::new();
    if let Some(p) = &a.person {
        m.insert("person".into(), json!(p));
    }
    m.insert("date".into(), json!(a.date.to_string()));
    if let Some(s) = &a.site {
        m.insert("site".into(), json!(s));
    }
    if let Some(s) = a.status {
        m.insert("status".into(), json!(status_name(s)));
    }
    if a.unstaffed {
        m.insert("unstaffed".into(), json!(true));
    }
    if let Some(p) = a.part {
        m.insert("part".into(), json!(p));
    }
    if let Some(s) = a.start {
        m.insert("start".into(), json!(fmt(s)));
    }
    if let Some(e) = a.end {
        m.insert("end".into(), json!(fmt(e)));
    }
    if let Some(n) = &a.note {
        m.insert("note".into(), json!(n));
    }
    Value::Object(m)
}

/// Lowercase, whitespace collapsed, accents kept.
fn norm(s: &str) -> String {
    spotter_core::text::clean(s).to_lowercase()
}

/// The comparable form of one assignment record (expected or actual).
fn tuple(v: &Value) -> String {
    let s = |k: &str| {
        v.get(k)
            .and_then(Value::as_str)
            .map(norm)
            .unwrap_or_default()
    };
    let unstaffed = v.get("unstaffed").and_then(Value::as_bool).unwrap_or(false);
    format!(
        "{} | person={} | site={} | status={} | unstaffed={} | part={} | time={}-{} | note={}",
        s("date"),
        s("person"),
        s("site"),
        s("status"),
        unstaffed,
        s("part"),
        s("start"),
        s("end"),
        s("note")
    )
}

/// Multiset difference: (missing from actual, extra in actual).
fn diff(expected: &[String], actual: &[String]) -> (Vec<String>, Vec<String>) {
    let mut counts: HashMap<&String, i64> = HashMap::new();
    for e in expected {
        *counts.entry(e).or_default() += 1;
    }
    for a in actual {
        *counts.entry(a).or_default() -= 1;
    }
    let mut missing = Vec::new();
    let mut extra = Vec::new();
    for (k, n) in counts {
        for _ in 0..n.max(0) {
            missing.push(k.clone());
        }
        for _ in 0..(-n).max(0) {
            extra.push(k.clone());
        }
    }
    missing.sort();
    extra.sort();
    (missing, extra)
}

fn settings_from(expected: &Value) -> Settings {
    let mut settings = Settings::default();
    let s = &expected["settings"];
    if let Some(t) = s.get("today").and_then(Value::as_str) {
        settings.today = NaiveDate::parse_from_str(t, "%Y-%m-%d").expect("settings.today");
    }
    if let Some(days) = s.get("workdays").and_then(Value::as_array) {
        let mut w = [false; 7];
        for d in days {
            let i = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
                .iter()
                .position(|x| Some(*x) == d.as_str());
            if let Some(i) = i {
                w[i] = true;
            }
        }
        settings.workdays = Workdays(w);
    }
    if let Some(off) = s.get("days_off").and_then(Value::as_array) {
        for d in off.iter().filter_map(Value::as_str) {
            if let Ok(d) = NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                settings.days_off.insert(d);
            }
        }
    }
    settings
}

/// Whether an actual question matches an expected question spec.
fn question_matches(spec: &Value, actual: &Value) -> bool {
    let Some(obj) = spec.as_object() else {
        return false;
    };
    if obj.get("type") != actual.get("type") {
        return false;
    }
    if let (Some(a), Some(b)) = (
        spec.get("a").and_then(Value::as_str),
        spec.get("b").and_then(Value::as_str),
    ) {
        let (xa, xb) = (
            actual.get("a").and_then(Value::as_str).unwrap_or(""),
            actual.get("b").and_then(Value::as_str).unwrap_or(""),
        );
        let same = |x: &str, y: &str| norm(x) == norm(y);
        if !((same(a, xa) && same(b, xb)) || (same(a, xb) && same(b, xa))) {
            return false;
        }
    }
    for (k, v) in obj {
        if matches!(k.as_str(), "type" | "a" | "b") {
            continue;
        }
        let Some(av) = actual.get(k) else {
            return false;
        };
        let ok = match (v, av) {
            (Value::String(x), Value::String(y)) => {
                norm(x) == norm(y) || norm(y).contains(&norm(x))
            }
            (Value::Array(xs), Value::Array(ys)) => xs.iter().all(|x| {
                ys.iter()
                    .any(|y| norm(&x.to_string()) == norm(&y.to_string()))
            }),
            (x, y) => x == y,
        };
        if !ok {
            return false;
        }
    }
    true
}

struct Outcome {
    id: String,
    category: String,
    tier: String,
    passed: bool,
    skipped: bool,
    problems: Vec<String>,
    millis: f64,
}

fn check_fixture(dir: &Path) -> Option<Outcome> {
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expected.json")).ok()?).ok()?;
    let id = dir.file_name()?.to_string_lossy().into_owned();
    let category = expected["category"].as_str().unwrap_or("?").to_string();
    let tier = expected["tier"].as_str().unwrap_or("script").to_string();
    let mut out = Outcome {
        id,
        category,
        tier: tier.clone(),
        passed: true,
        skipped: false,
        problems: Vec::new(),
        millis: 0.0,
    };
    let input_name = expected["input"].as_str().unwrap_or("input.csv");
    let paste = expected["paste"].as_bool().unwrap_or(false);
    let settings = settings_from(&expected);
    let input = load_input(&dir.join(input_name), paste);
    let started = Instant::now();
    let report = analyze(&input, &settings);
    out.millis = started.elapsed().as_secs_f64() * 1000.0;
    let questions: Vec<Value> = report
        .reading
        .questions
        .iter()
        .map(|q| serde_json::to_value(q).expect("json"))
        .collect();

    // A model-tier fixture with the model off: only the questions it must
    // fall back to are checked.
    if tier == "model" {
        out.skipped = true;
        return Some(out);
    }

    let mut problem = |p: String| {
        out.passed = false;
        out.problems.push(p);
    };
    match (expected.get("error").and_then(Value::as_str), report.error) {
        (Some(e), Some(actual)) => {
            let actual = serde_json::to_value(actual).expect("json");
            if actual.as_str() != Some(e) {
                problem(format!("error: expected {e}, got {actual}"));
            }
            return Some(out);
        }
        (Some(e), None) => {
            problem(format!("error: expected {e}, got a reading"));
            return Some(out);
        }
        (None, Some(actual)) => {
            problem(format!("unexpected error {actual:?}"));
            return Some(out);
        }
        (None, None) => {}
    }
    if let Some(layout) = expected.get("layout").and_then(Value::as_str) {
        let actual = serde_json::to_value(report.reading.layout).expect("json");
        if actual.as_str() != Some(layout) {
            problem(format!("layout: expected {layout}, got {actual}"));
        }
    }
    if let Some(arr) = expected.get("arrangement").and_then(Value::as_str) {
        let actual = serde_json::to_value(report.reading.arrangement).expect("json");
        if actual.as_str() != Some(arr) {
            problem(format!("arrangement: expected {arr}, got {actual}"));
        }
    }
    if let Some(exp) = expected.get("assignments").and_then(Value::as_array) {
        let exact = expected
            .get("assignments_exact")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let e: Vec<String> = exp.iter().map(tuple).collect();
        let a: Vec<String> = report
            .reading
            .assignments
            .iter()
            .map(|x| tuple(&assignment_json(x)))
            .collect();
        let (missing, extra) = diff(&e, &a);
        for m in &missing {
            problem(format!("missing: {m}"));
        }
        if exact {
            for x in &extra {
                problem(format!("extra:   {x}"));
            }
        }
    }
    for spec in expected
        .get("ask")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if !questions.iter().any(|q| question_matches(spec, q)) {
            problem(format!("not asked: {spec}"));
        }
    }
    for spec in expected
        .get("must_not_ask")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(q) = questions.iter().find(|q| question_matches(spec, q)) {
            problem(format!("asked but must not: {q}"));
        }
    }
    for note in expected
        .get("notes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        let all = report.reading.notes.join(" ").to_lowercase();
        if !all.contains(&note.to_lowercase()) {
            problem(format!(
                "note missing: {note:?} (notes: {:?})",
                report.reading.notes
            ));
        }
    }
    Some(out)
}

fn fixture_dirs(roots: &[String]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.join("expected.json").exists() {
                dirs.push(p);
            }
        }
    }
    dirs.sort();
    dirs
}

fn cmd_fixtures(args: &[String]) -> i32 {
    let roots: Vec<String> = args
        .iter()
        .take_while(|a| !a.starts_with("--"))
        .cloned()
        .collect();
    let only = flag(args, "--only");
    let verbose = args.iter().any(|a| a == "--verbose");
    let mut outcomes = Vec::new();
    for dir in fixture_dirs(&roots) {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if only.as_ref().is_some_and(|o| !name.starts_with(o.as_str())) {
            continue;
        }
        match check_fixture(&dir) {
            Some(o) => outcomes.push(o),
            None => eprintln!("could not load {}", dir.display()),
        }
    }
    let mut by_cat: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    for o in &outcomes {
        let e = by_cat.entry(o.category.clone()).or_default();
        if o.skipped {
            e.2 += 1;
        } else {
            e.1 += 1;
            if o.passed {
                e.0 += 1;
            }
        }
        if !o.passed && !o.skipped {
            println!("FAIL {} ({:.1} ms)", o.id, o.millis);
            let shown = if verbose {
                o.problems.len()
            } else {
                o.problems.len().min(6)
            };
            for p in &o.problems[..shown] {
                println!("    {p}");
            }
            if shown < o.problems.len() {
                println!("    … {} more", o.problems.len() - shown);
            }
        }
    }
    println!("\ncategory  pass/total  (model-tier skipped)");
    let (mut pass, mut total) = (0, 0);
    for (cat, (p, t, s)) in &by_cat {
        println!("  {cat:<6}  {p:>3}/{t:<3}     ({s})");
        pass += p;
        total += t;
    }
    println!("  all     {pass:>3}/{total:<3}");
    if let Some(path) = flag(args, "--json") {
        let rows: Vec<Value> = outcomes
            .iter()
            .map(|o| json!({"id": o.id, "category": o.category, "tier": o.tier, "passed": o.passed, "skipped": o.skipped, "problems": o.problems, "ms": o.millis}))
            .collect();
        std::fs::write(path, serde_json::to_string_pretty(&rows).expect("json"))
            .expect("write --json");
    }
    if pass == total { 0 } else { 1 }
}

fn cmd_generated(args: &[String]) -> i32 {
    let Some(root) = args.first() else { return 2 };
    let verbose = args.iter().any(|a| a == "--verbose");
    let min_recall: f64 = flag(args, "--min-recall")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.99);
    let (mut exp_total, mut matched_total, mut act_total) = (0usize, 0usize, 0usize);
    let mut perfect = 0usize;
    let mut cases = 0usize;
    let mut worst: Vec<(f64, String, Vec<String>, Vec<String>)> = Vec::new();
    let mut times = Vec::new();
    let mut report_rows: Vec<Value> = Vec::new();
    for dir in fixture_dirs(std::slice::from_ref(root)) {
        let Ok(text) = std::fs::read_to_string(dir.join("expected.json")) else {
            continue;
        };
        let Ok(expected) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let input_name = expected["input"].as_str().unwrap_or("input.csv");
        let input = load_input(
            &dir.join(input_name),
            expected["paste"].as_bool().unwrap_or(false),
        );
        let settings = settings_from(&expected);
        let started = Instant::now();
        let report = analyze(&input, &settings);
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        let e: Vec<String> = expected["assignments"]
            .as_array()
            .map(|v| v.iter().map(tuple).collect())
            .unwrap_or_default();
        let a: Vec<String> = report
            .reading
            .assignments
            .iter()
            .map(|x| tuple(&assignment_json(x)))
            .collect();
        let (missing, extra) = diff(&e, &a);
        let matched = e.len() - missing.len();
        exp_total += e.len();
        act_total += a.len();
        matched_total += matched;
        cases += 1;
        if missing.is_empty() {
            perfect += 1;
        }
        let recall = if e.is_empty() {
            1.0
        } else {
            matched as f64 / e.len() as f64
        };
        report_rows.push(json!({
            "case": dir.file_name().map(|n| n.to_string_lossy().into_owned()),
            "layout": expected["layout"],
            "input": expected["input"],
            "expected": e.len(),
            "actual": a.len(),
            "missing": missing,
            "extra": extra,
            "got_layout": report.reading.layout,
        }));
        if recall < 1.0 || !extra.is_empty() {
            let name = format!(
                "{} ({}, {})",
                dir.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                expected["layout"].as_str().unwrap_or("?"),
                expected["input"].as_str().unwrap_or("?")
            );
            worst.push((recall, name, missing, extra));
        }
    }
    let recall = if exp_total == 0 {
        1.0
    } else {
        matched_total as f64 / exp_total as f64
    };
    let precision = if act_total == 0 {
        1.0
    } else {
        matched_total as f64 / act_total as f64
    };
    worst.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (r, name, missing, extra) in worst.iter().take(if verbose { 60 } else { 15 }) {
        println!(
            "{:5.1}%  {name}  missing {} extra {}",
            r * 100.0,
            missing.len(),
            extra.len()
        );
        if verbose {
            for m in missing.iter().take(4) {
                println!("        missing {m}");
            }
            for x in extra.iter().take(4) {
                println!("        extra   {x}");
            }
        }
    }
    if let Some(path) = flag(args, "--report") {
        std::fs::write(path, serde_json::to_string(&report_rows).expect("json"))
            .expect("write --report");
    }
    times.sort_by(f64::total_cmp);
    let p95 = times
        .get(times.len().saturating_sub(1) * 95 / 100)
        .copied()
        .unwrap_or(0.0);
    println!(
        "\ncases {cases}  assignment recall {:.3}%  precision {:.3}%  perfect cases {perfect}/{cases}  p95 {p95:.1} ms",
        recall * 100.0,
        precision * 100.0
    );
    if recall >= min_recall { 0 } else { 1 }
}
