//! Model benchmark harness: loads one model variant, runs the labeled
//! question set (bench/questions/*.jsonl), and prints the metrics the spec's
//! thresholds are measured on.
//!
//!   spotter-bench --model models/X.gguf --variant C [--sha HEX] [--threads 2]
//!                 [--questions bench/questions] [--limit 5] [--out result.json]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use spotter_core::matching::{Tiebreaker, Verdict};
use spotter_llm::{Engine, Task};

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn peak_rss_mb() -> f64 {
    #[cfg(unix)]
    {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
        // SAFETY: getrusage fills the struct for the calling process.
        unsafe {
            if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) == 0 {
                let max = usage.assume_init().ru_maxrss as f64;
                return if cfg!(target_os = "macos") {
                    max / 1048576.0
                } else {
                    max / 1024.0
                };
            }
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        let mut c: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        // SAFETY: pseudo-handle for this process and a correctly sized struct.
        if unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) } != 0 {
            return c.PeakWorkingSetSize as f64 / 1048576.0;
        }
    }
    #[allow(unreachable_code)]
    0.0
}

/// The labeled questions. Controls the rules settle on their own in the app
/// (`kind: control_never_asked`, e.g. Mike B / Mike R) stay in: the spec
/// names them as part of the set.
fn read_jsonl(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

#[derive(Default)]
struct Tally {
    items: usize,
    answered: usize,
    correct: usize,
    unsure: usize,
    false_merges: usize,
    different_items: usize,
    latencies: Vec<f64>,
    wrong: Vec<String>,
    /// One record per question: id, kind, and right / wrong / unsure.
    records: Vec<Value>,
}

impl Tally {
    fn record(&mut self, q: &Value, outcome: &str, said: &str) {
        self.records.push(json!({
            "id": q["id"], "kind": q["kind"], "outcome": outcome, "said": said,
            "latency_s": self.latencies.last().copied().unwrap_or(0.0),
        }));
    }

    fn json(&self) -> Value {
        let mut l = self.latencies.clone();
        l.sort_by(f64::total_cmp);
        let pct = |p: f64| {
            l.get(((l.len().max(1) - 1) as f64 * p) as usize)
                .copied()
                .unwrap_or(0.0)
        };
        json!({
            "items": self.items,
            "answered": self.answered,
            "accuracy_on_answered": if self.answered == 0 { 0.0 } else { self.correct as f64 / self.answered as f64 },
            "unsure_rate": if self.items == 0 { 0.0 } else { self.unsure as f64 / self.items as f64 },
            "false_merges": self.false_merges,
            "false_merge_rate": if self.different_items == 0 { 0.0 } else { self.false_merges as f64 / self.different_items as f64 },
            "latency_p50_s": pct(0.5),
            "latency_p95_s": pct(0.95),
            "wrong_examples": self.wrong.iter().take(12).collect::<Vec<_>>(),
        })
    }
}

fn norm(s: &str) -> String {
    spotter_core::text::key(s)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let model = PathBuf::from(flag(&args, "--model").expect("--model path"));
    let variant = flag(&args, "--variant").unwrap_or_else(|| "C".into());
    let threads: i32 = flag(&args, "--threads")
        .and_then(|t| t.parse().ok())
        .unwrap_or_else(spotter_llm::default_threads);
    let questions =
        PathBuf::from(flag(&args, "--questions").unwrap_or_else(|| "bench/questions".into()));
    let limit = Duration::from_secs_f64(
        flag(&args, "--limit")
            .and_then(|l| l.parse().ok())
            .unwrap_or(5.0),
    );
    let only: Option<usize> = flag(&args, "--max-per-task").and_then(|n| n.parse().ok());
    let same_min: f32 = flag(&args, "--same-min")
        .and_then(|n| n.parse().ok())
        .unwrap_or(spotter_llm::SAME_MIN);

    let rss_before = peak_rss_mb();
    let engine = match Engine::load(&model, flag(&args, "--sha").as_deref(), &variant, threads) {
        Ok(e) => e.with_limit(limit),
        Err(e) => {
            println!("{}", json!({"variant": variant, "error": e.to_string()}));
            std::process::exit(1);
        }
    };
    let load_s = engine.load_secs;
    // As in the app: task prefixes are evaluated on the loading thread
    // before the first question.
    let warm_s = engine.warm(&spotter_llm::Task::ALL);

    let mut tallies: Vec<(&str, Tally)> = Vec::new();
    let take = |v: Vec<Value>| -> Vec<Value> {
        match only {
            Some(n) => v.into_iter().take(n).collect(),
            None => v,
        }
    };

    for (name, file) in [
        ("t1_same_person", "t1_same_person.jsonl"),
        ("t2_same_site", "t2_same_site.jsonl"),
    ] {
        let mut t = Tally::default();
        for q in take(read_jsonl(&questions.join(file))) {
            let (a, b) = (q["a"].as_str().unwrap_or(""), q["b"].as_str().unwrap_or(""));
            let others = if name.starts_with("t1") {
                strings(&q["roster"])
            } else {
                strings(&q["sites"])
            };
            let start = Instant::now();
            let task = if name.starts_with("t1") { Task::SamePerson } else { Task::SameSite };
            let raw = engine.same_answer(task, a, b, &others);
            let confidence = raw.as_ref().map_or(0.0, |r| r.1);
            let v = match raw.as_ref().map(|r| r.0.as_str()) {
                Some("same") if confidence >= same_min => Verdict::Same,
                Some("different") => Verdict::Different,
                _ => Verdict::Unsure,
            };
            t.latencies.push(start.elapsed().as_secs_f64());
            t.items += 1;
            let label = q["label"].as_str().unwrap_or("");
            if label == "different" {
                t.different_items += 1;
            }
            match v {
                Verdict::Unsure => {
                    t.unsure += 1;
                    t.record(&q, "unsure", "unsure");
                }
                v => {
                    t.answered += 1;
                    let said = if v == Verdict::Same {
                        "same"
                    } else {
                        "different"
                    };
                    t.record(&q, if said == label { "right" } else { "wrong" }, said);
                    if said == label {
                        t.correct += 1;
                    } else {
                        t.wrong.push(format!(
                            "{}: {a} / {b} -> {said}",
                            q["id"].as_str().unwrap_or("")
                        ));
                        if said == "same" {
                            t.false_merges += 1;
                        }
                    }
                }
            }
            // The model's own answer and confidence, before the confidence
            // rule, so MODEL_REPORT.md can show the trade-off.
            if let Some(r) = t.records.last_mut() {
                r["raw"] = json!(raw.as_ref().map(|r| r.0.clone()));
                r["confidence"] = json!(confidence);
            }
        }
        tallies.push((name, t));
    }

    for (name, file) in [
        ("t3_cell_meaning", "t3_cell_meaning.jsonl"),
        ("t4_column_role", "t4_column_role.jsonl"),
    ] {
        let mut t = Tally::default();
        for q in take(read_jsonl(&questions.join(file))) {
            let start = Instant::now();
            let got = if name.starts_with("t3") {
                engine.cell_meaning(
                    q["text"].as_str().unwrap_or(""),
                    q["column_role"].as_str().unwrap_or(""),
                    &strings(&q["neighbors"]),
                )
            } else {
                engine.column_role(q["header"].as_str().unwrap_or(""), &strings(&q["samples"]))
            };
            t.latencies.push(start.elapsed().as_secs_f64());
            t.items += 1;
            match got {
                None => {
                    t.unsure += 1;
                    t.record(&q, "unsure", "unsure");
                }
                Some(g) => {
                    t.answered += 1;
                    let right = Some(g.as_str()) == q["label"].as_str();
                    t.record(&q, if right { "right" } else { "wrong" }, &g);
                    if right {
                        t.correct += 1;
                    } else {
                        t.wrong.push(format!(
                            "{}: {} -> {g}",
                            q["id"].as_str().unwrap_or(""),
                            q["label"]
                        ));
                    }
                }
            }
        }
        tallies.push((name, t));
    }

    {
        let mut t = Tally::default();
        for q in take(read_jsonl(&questions.join("t5_free_text.jsonl"))) {
            let line = q["line"].as_str().unwrap_or("");
            let start = Instant::now();
            let got = engine.free_text(line, &strings(&q["roster"]), &strings(&q["sites"]));
            t.latencies.push(start.elapsed().as_secs_f64());
            t.items += 1;
            let canon = |person: &str, day: &str, site: &str, time: Option<&str>| {
                format!(
                    "{}|{}|{}|{}",
                    norm(person),
                    day,
                    norm(site),
                    time.unwrap_or("-")
                )
            };
            let expected: Option<Vec<String>> = q["label"].as_array().map(|a| {
                let mut v: Vec<String> = a
                    .iter()
                    .map(|e| {
                        canon(
                            e["person"].as_str().unwrap_or(""),
                            e["day"].as_str().unwrap_or(""),
                            e["site"].as_str().unwrap_or(""),
                            e["time"].as_str(),
                        )
                    })
                    .collect();
                v.sort();
                v
            });
            let actual: Option<Vec<String>> = got.map(|es| {
                let mut v: Vec<String> = es
                    .iter()
                    .map(|e| canon(&e.person, &e.day, &e.site, e.time.as_deref()))
                    .collect();
                v.sort();
                v
            });
            // "cannot read" answered correctly counts as answered and right.
            t.answered += 1;
            let said = actual
                .as_ref()
                .map_or("cannot read".to_string(), |v| v.join("; "));
            t.record(
                &q,
                if expected == actual { "right" } else { "wrong" },
                &said,
            );
            if expected == actual {
                t.correct += 1;
            } else {
                t.wrong
                    .push(format!("{}: {line}", q["id"].as_str().unwrap_or("")));
            }
        }
        tallies.push(("t5_free_text", t));
    }

    let (mut items, mut answered, mut correct, mut unsure, mut fm, mut diff) = (0, 0, 0, 0, 0, 0);
    let mut all_lat: Vec<f64> = Vec::new();
    for (_, t) in &tallies {
        items += t.items;
        answered += t.answered;
        correct += t.correct;
        unsure += t.unsure;
        fm += t.false_merges;
        diff += t.different_items;
        all_lat.extend(&t.latencies);
    }
    all_lat.sort_by(f64::total_cmp);
    let pct = |p: f64| {
        all_lat
            .get(((all_lat.len().max(1) - 1) as f64 * p) as usize)
            .copied()
            .unwrap_or(0.0)
    };
    let result = json!({
        "variant": variant,
        "model": model.file_name().map(|n| n.to_string_lossy().into_owned()),
        "threads": threads,
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "same_min": same_min,
        "load_s": load_s,
        "warm_s": warm_s,
        "peak_rss_mb": peak_rss_mb(),
        "rss_before_load_mb": rss_before,
        "overall": {
            "items": items,
            "accuracy_on_answered": if answered == 0 { 0.0 } else { correct as f64 / answered as f64 },
            "unsure_rate": if items == 0 { 0.0 } else { unsure as f64 / items as f64 },
            "false_merge_rate": if diff == 0 { 0.0 } else { fm as f64 / diff as f64 },
            "latency_p50_s": pct(0.5),
            "latency_p95_s": pct(0.95),
        },
        "tasks": tallies.iter().map(|(n, t)| (n.to_string(), t.json())).collect::<serde_json::Map<String, Value>>(),
        "records": tallies.iter().flat_map(|(_, t)| t.records.iter().cloned()).collect::<Vec<_>>(),
    });
    let text = serde_json::to_string_pretty(&result).expect("json");
    println!("{text}");
    if let Some(out) = flag(&args, "--out") {
        std::fs::write(out, text).expect("write --out");
    }
}
