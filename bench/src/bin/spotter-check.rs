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
use spotter_core::{
    Answer, Answers, Input, NoModel, Question, Report, Settings, Tiebreaker, analyze_with,
};

/// The model under test (`--model PATH --variant X`), if any.
static MODEL: std::sync::OnceLock<Option<spotter_llm::Engine>> = std::sync::OnceLock::new();

fn model() -> Option<&'static spotter_llm::Engine> {
    MODEL.get().and_then(Option::as_ref)
}

/// The tiebreaker runs use: the model under test, or none.
fn tiebreak() -> &'static dyn Tiebreaker {
    match model() {
        Some(m) => m,
        None => &NoModel,
    }
}

fn analyze(input: &Input, settings: &Settings) -> Report {
    analyze_with(input, settings, &Answers::default(), tiebreak())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let engine = flag(&args, "--model").map(|path| {
        let variant = flag(&args, "--variant").unwrap_or_else(|| "C".into());
        let threads = flag(&args, "--threads")
            .and_then(|t| t.parse().ok())
            .unwrap_or_else(spotter_llm::default_threads);
        match spotter_llm::Engine::load(
            Path::new(&path),
            flag(&args, "--sha").as_deref(),
            &variant,
            threads,
        ) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("model {path}: {e}");
                std::process::exit(2);
            }
        }
    });
    // As in the app: task prefixes are evaluated right after loading.
    if let Some(e) = &engine {
        e.warm(&spotter_llm::Task::ALL);
    }
    let _ = MODEL.set(engine);
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
        "questions": report.questions,
        "merges": report.merges,
        "flags": report.flags.iter().map(flag_json).collect::<Vec<_>>(),
        "info": report.info.iter().map(info_json).collect::<Vec<_>>(),
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

/// The comparable form of a flag or info item: type, date, who, where.
fn finding_tuple(v: &Value) -> String {
    let s = |k: &str| {
        v.get(k)
            .and_then(Value::as_str)
            .map(norm)
            .unwrap_or_default()
    };
    let mut sites: Vec<String> = v
        .get("sites")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(norm).collect())
        .unwrap_or_default();
    sites.sort();
    format!(
        "{} {} | person={} | site={} | sites={} | status={}",
        s("type"),
        s("date"),
        s("person"),
        s("site"),
        sites.join("; "),
        s("status")
    )
}

fn flag_json(f: &spotter_core::Flag) -> Value {
    let mut v = serde_json::to_value(f).expect("json");
    let kind = v["kind"].clone();
    v["type"] = kind;
    v
}

fn info_json(i: &spotter_core::model::Info) -> Value {
    let mut v = serde_json::to_value(i).expect("json");
    let kind = v["kind"].clone();
    v["type"] = kind;
    v
}

/// Compares expected findings with actual ones; `exact` also reports extras.
fn compare_findings(
    label: &str,
    expected: &[Value],
    actual: &[Value],
    exact: bool,
    problem: &mut dyn FnMut(String),
) {
    let e: Vec<String> = expected.iter().map(finding_tuple).collect();
    let a: Vec<String> = actual.iter().map(finding_tuple).collect();
    let (missing, extra) = diff(&e, &a);
    for m in &missing {
        problem(format!("{label} missing: {m}"));
    }
    if exact {
        for x in &extra {
            problem(format!("{label} extra:   {x}"));
        }
    }
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
    // Script tier always runs with the model off: it defines the rules'
    // behavior. Model tier runs with the model under test.
    let report = if tier == "model" {
        analyze(&input, &settings)
    } else {
        analyze_with(&input, &settings, &Answers::default(), &NoModel)
    };
    out.millis = started.elapsed().as_secs_f64() * 1000.0;
    let questions: Vec<Value> = report
        .questions
        .iter()
        .map(|q| serde_json::to_value(q).expect("json"))
        .collect();

    // A model-tier fixture needs the model; without one it's skipped.
    let model_run = tier == "model" && model().is_some();
    if tier == "model" && !model_run {
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
    if let Some(exp) = expected.get("flags").and_then(Value::as_array) {
        let exact = expected
            .get("flags_exact")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let actual: Vec<Value> = report.flags.iter().map(flag_json).collect();
        compare_findings("flag", exp, &actual, exact, &mut problem);
    }
    if let Some(exp) = expected.get("info").and_then(Value::as_array) {
        let exact = expected
            .get("info_exact")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let actual: Vec<Value> = report.info.iter().map(info_json).collect();
        compare_findings("info", exp, &actual, exact, &mut problem);
    }
    for spec in expected
        .get("ask")
        .filter(|_| !model_run)
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

    // The owner answers; results update.
    if let Some(given) = expected.get("answers").and_then(Value::as_array) {
        let answers = answers_from(given, &report.questions);
        let after = analyze_with(
            &input,
            &settings,
            &answers,
            if model_run { tiebreak() } else { &NoModel },
        );
        let target = &expected["after_answers"];
        if let Some(exp) = target.get("assignments").and_then(Value::as_array) {
            let e: Vec<String> = exp.iter().map(tuple).collect();
            let a: Vec<String> = after
                .reading
                .assignments
                .iter()
                .map(|x| tuple(&assignment_json(x)))
                .collect();
            let (missing, extra) = diff(&e, &a);
            for m in &missing {
                problem(format!("after answers, missing: {m}"));
            }
            if target
                .get("assignments_exact")
                .and_then(Value::as_bool)
                .unwrap_or(true)
            {
                for x in &extra {
                    problem(format!("after answers, extra:   {x}"));
                }
            }
        }
        if let Some(exp) = target.get("flags").and_then(Value::as_array) {
            let actual: Vec<Value> = after.flags.iter().map(flag_json).collect();
            compare_findings("after answers, flag", exp, &actual, true, &mut problem);
        }
        if let Some(exp) = target.get("info").and_then(Value::as_array) {
            let exact = target
                .get("info_exact")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let actual: Vec<Value> = after.info.iter().map(info_json).collect();
            compare_findings("after answers, info", exp, &actual, exact, &mut problem);
        }
    }
    Some(out)
}

/// Turns expected.json answers into the app's answers. Each spec is matched
/// to a question actually asked, as the owner would tap it; a same-person or
/// same-site answer with no question (undoing a merge chip) is built from
/// its spec.
fn answers_from(given: &[Value], asked: &[Question]) -> Answers {
    let mut answers = Answers::default();
    for item in given {
        let spec = &item["question"];
        let answer = match item["answer"].as_str().unwrap_or("") {
            "yes" => Answer::Yes,
            "no" => Answer::No,
            other => Answer::Pick(other.to_string()),
        };
        let found = asked
            .iter()
            .find(|q| question_matches(spec, &serde_json::to_value(q).expect("json")))
            .cloned();
        let question = found.or_else(|| {
            let (a, b) = (
                spec["a"].as_str()?.to_string(),
                spec["b"].as_str()?.to_string(),
            );
            match spec["type"].as_str()? {
                "same_person" => Some(Question::SamePerson { a, b }),
                "same_site" => Some(Question::SameSite { a, b }),
                _ => None,
            }
        });
        match question {
            Some(q) => answers.items.push((q, answer)),
            None => eprintln!("answer for a question that wasn't asked: {spec}"),
        }
    }
    answers
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
    // Known-failing fixtures still run and are reported, but don't fail the
    // run; one that starts passing is called out so it leaves the list.
    let known: Vec<String> = flag(args, "--known-failing")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| {
            t.lines()
                .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default();
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
    let mut unexpected = 0;
    for o in outcomes.iter().filter(|o| !o.skipped) {
        let is_known = known.iter().any(|k| o.id.starts_with(k.as_str()));
        match (o.passed, is_known) {
            (false, true) => println!("  known failing (TEST_REPORT.md): {}", o.id),
            (true, true) => println!(
                "  NOW PASSING, remove from the known-failing list: {}",
                o.id
            ),
            (false, false) => unexpected += 1,
            (true, false) => {}
        }
    }
    if let Some(path) = flag(args, "--json") {
        let rows: Vec<Value> = outcomes
            .iter()
            .map(|o| json!({"id": o.id, "category": o.category, "tier": o.tier, "passed": o.passed, "skipped": o.skipped, "problems": o.problems, "ms": o.millis}))
            .collect();
        std::fs::write(path, serde_json::to_string_pretty(&rows).expect("json"))
            .expect("write --json");
    }
    if unexpected == 0 { 0 } else { 1 }
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
    // Per flag type: (expected, matched, actual).
    let mut flag_stats: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    let mut clean_cases = 0usize;
    let mut clean_with_red: Vec<String> = Vec::new();
    let mut false_merges: Vec<String> = Vec::new();
    let mut merges_total = 0usize;
    let mut question_counts: Vec<usize> = Vec::new();
    let mut match_questions = 0usize;
    // Flags after an oracle owner answers every same-person/same-site
    // question from the truth: (expected, matched, actual) per type.
    let mut answered_stats: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
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

        let exp_flags: Vec<String> = expected["flags"]
            .as_array()
            .map(|v| v.iter().map(finding_tuple).collect())
            .unwrap_or_default();
        let act_flags: Vec<String> = report
            .flags
            .iter()
            .map(|f| finding_tuple(&flag_json(f)))
            .collect();
        let (flags_missing, flags_extra) = diff(&exp_flags, &act_flags);
        for (list, slot) in [(&exp_flags, 0), (&act_flags, 2)] {
            for f in list {
                let kind = f.split(' ').next().unwrap_or("").to_string();
                let e = flag_stats.entry(kind).or_default();
                if slot == 0 { e.0 += 1 } else { e.2 += 1 }
            }
        }
        let mut matched_flags = exp_flags.clone();
        for m in &flags_missing {
            if let Some(i) = matched_flags.iter().position(|x| x == m) {
                matched_flags.remove(i);
            }
        }
        for f in &matched_flags {
            let kind = f.split(' ').next().unwrap_or("").to_string();
            flag_stats.entry(kind).or_default().1 += 1;
        }
        let truth: Value = std::fs::read_to_string(dir.join("truth.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null);
        // Which spellings are which truth identity.
        let alias_ids = |list: &Value| -> HashMap<String, String> {
            let mut m = HashMap::new();
            for item in list.as_array().into_iter().flatten() {
                let id = item["id"].as_str().unwrap_or("").to_string();
                for name in item["aliases"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .chain(item["name"].as_str())
                {
                    m.insert(norm(name), id.clone());
                }
            }
            m
        };
        let people = alias_ids(&truth["people"]);
        let sites = alias_ids(&truth["sites"]);
        for merge in &report.merges {
            merges_total += 1;
            let ids = match merge.kind {
                spotter_core::matching::MergeKind::Person => &people,
                spotter_core::matching::MergeKind::Site => &sites,
            };
            if let (Some(a), Some(b)) = (ids.get(&norm(&merge.from)), ids.get(&norm(&merge.into)))
                && a != b
            {
                false_merges.push(format!(
                    "{}: {:?} {:?} -> {:?}",
                    dir.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    merge.kind,
                    merge.from,
                    merge.into
                ));
            }
        }
        question_counts.push(report.questions.len());
        let mut oracle = Answers::default();
        for q in &report.questions {
            let (ids, a, b) = match q {
                Question::SamePerson { a, b } => (&people, a, b),
                Question::SameSite { a, b } => (&sites, a, b),
                _ => continue,
            };
            match_questions += 1;
            let same =
                matches!((ids.get(&norm(a)), ids.get(&norm(b))), (Some(x), Some(y)) if x == y);
            oracle
                .items
                .push((q.clone(), if same { Answer::Yes } else { Answer::No }));
        }
        let after = if oracle.is_empty() {
            report.clone()
        } else {
            analyze_with(&input, &settings, &oracle, tiebreak())
        };
        let after_flags: Vec<String> = after
            .flags
            .iter()
            .map(|f| finding_tuple(&flag_json(f)))
            .collect();
        let (after_missing, after_extra) = diff(&exp_flags, &after_flags);
        for (list, slot) in [(&exp_flags, 0), (&after_flags, 2)] {
            for f in list {
                let kind = f.split(' ').next().unwrap_or("").to_string();
                let e = answered_stats.entry(kind).or_default();
                if slot == 0 { e.0 += 1 } else { e.2 += 1 }
            }
        }
        let mut matched_after = exp_flags.clone();
        for m in &after_missing {
            if let Some(i) = matched_after.iter().position(|x| x == m) {
                matched_after.remove(i);
            }
        }
        for f in &matched_after {
            answered_stats
                .entry(f.split(' ').next().unwrap_or("").to_string())
                .or_default()
                .1 += 1;
        }

        if truth["clean"].as_bool() == Some(true) {
            clean_cases += 1;
            let red: Vec<&String> = act_flags
                .iter()
                .filter(|f| f.starts_with("double_booked") || f.starts_with("assigned_while_off"))
                .collect();
            if !red.is_empty() {
                clean_with_red.push(format!(
                    "{} ({}): {}",
                    dir.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    expected["layout"].as_str().unwrap_or("?"),
                    red[0]
                ));
            }
        }

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
            "flags_missing": flags_missing,
            "flags_extra": flags_extra,
            "after_missing": after_missing,
            "after_extra": after_extra,
            "clean": truth["clean"],
            "tier": expected["tier"],
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
    println!("\nflags            expected  matched   actual   recall  precision");
    for (kind, (e, m, a)) in &flag_stats {
        let pct = |x: usize, y: usize| {
            if y == 0 {
                100.0
            } else {
                100.0 * x as f64 / y as f64
            }
        };
        println!(
            "  {kind:<18} {e:>7}  {m:>7}  {a:>7}  {:>6.2}%  {:>6.2}%",
            pct(*m, *e),
            pct(*m, *a)
        );
    }
    println!(
        "clean schedules {clean_cases}, with a red flag: {}",
        clean_with_red.len()
    );
    for c in clean_with_red.iter().take(if verbose { 40 } else { 8 }) {
        println!("    {c}");
    }
    println!("\nafter the owner answers every same-person/same-site question correctly:");
    for (kind, (e, m, a)) in &answered_stats {
        let pct = |x: usize, y: usize| {
            if y == 0 {
                100.0
            } else {
                100.0 * x as f64 / y as f64
            }
        };
        println!(
            "  {kind:<18} {e:>7}  {m:>7}  {a:>7}  {:>6.2}%  {:>6.2}%",
            pct(*m, *e),
            pct(*m, *a)
        );
    }
    let files = question_counts.len().max(1) as f64;
    let questions_mean = question_counts.iter().sum::<usize>() as f64 / files;
    println!(
        "questions per file (model {}): mean {:.2}, files with none {}/{}, same-person/site questions {match_questions}",
        if model().is_some() { "on" } else { "off" },
        questions_mean,
        question_counts.iter().filter(|n| **n == 0).count(),
        question_counts.len()
    );
    println!("merges {merges_total}, false merges {}", false_merges.len());
    for f in false_merges.iter().take(if verbose { 40 } else { 10 }) {
        println!("    {f}");
    }
    let merge_gate =
        args.iter().any(|a| a == "--require-no-false-merges") && !false_merges.is_empty();
    let flag_gate = args.iter().any(|a| a == "--require-clean");
    if let Some(path) = flag(args, "--report") {
        std::fs::write(path, serde_json::to_string(&report_rows).expect("json"))
            .expect("write --report");
    }
    if let Some(path) = flag(args, "--summary") {
        let summary = json!({
            "model": model().is_some(),
            "files": question_counts.len(),
            "questions_per_file": questions_mean,
            "files_without_questions": question_counts.iter().filter(|n| **n == 0).count(),
            "match_questions": match_questions,
            "merges": merges_total,
            "false_merges": false_merges.len(),
            "clean_with_red_flag": clean_with_red.len(),
        });
        std::fs::write(path, serde_json::to_string_pretty(&summary).expect("json"))
            .expect("write --summary");
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
    if recall >= min_recall && !(flag_gate && !clean_with_red.is_empty()) && !merge_gate {
        0
    } else {
        1
    }
}
