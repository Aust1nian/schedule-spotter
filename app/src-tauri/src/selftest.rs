//! `--self-test`: opens the real window, renders the built-in example, checks
//! the page shows what the core found, prints JSON metrics and exits nonzero
//! if any threshold is missed. CI runs it on every runner.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use spotter_core::Settings;

use crate::metrics;

/// Give up and fail if the page hasn't reported back by then.
const WATCHDOG: Duration = Duration::from_secs(90);
/// Whole-app ceiling with the model not loaded (spec: Hard constraints).
const MAX_RSS_MODEL_OFF_MB: f64 = 300.0;
/// The window must show its first page within this long.
const MAX_PAGE_READY_MS: u128 = 15_000;

pub struct Args {
    pub self_test: bool,
    pub json_out: Option<PathBuf>,
}

impl Args {
    pub fn parse(mut args: impl Iterator<Item = String>) -> Args {
        let mut parsed = Args {
            self_test: false,
            json_out: None,
        };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--self-test" => parsed.self_test = true,
                "--json-out" => parsed.json_out = args.next().map(PathBuf::from),
                _ => {}
            }
        }
        parsed
    }
}

pub struct SelfTest {
    json_out: Option<PathBuf>,
    page_ready_ms: OnceLock<u128>,
    done: Arc<AtomicBool>,
}

impl SelfTest {
    pub fn start(json_out: Option<PathBuf>) -> SelfTest {
        let done = Arc::new(AtomicBool::new(false));
        let watchdog_done = Arc::clone(&done);
        let watchdog_out = json_out.clone();
        std::thread::spawn(move || {
            std::thread::sleep(WATCHDOG);
            if !watchdog_done.swap(true, Ordering::SeqCst) {
                let report = json!({
                    "self_test": "fail",
                    "failures": [format!("page did not report back within {} s", WATCHDOG.as_secs())],
                });
                emit(&report, watchdog_out.as_ref());
                std::process::exit(3);
            }
        });
        SelfTest {
            json_out,
            page_ready_ms: OnceLock::new(),
            done,
        }
    }

    pub fn mark_page_ready(&self, started: Instant) {
        let _ = self.page_ready_ms.set(started.elapsed().as_millis());
    }

    /// Checks thresholds, prints the metrics, and returns the exit code.
    pub fn finish(&self, started: Instant, rendered_flags: usize) -> i32 {
        if self.done.swap(true, Ordering::SeqCst) {
            return 3;
        }
        let settings = Settings::default();
        let core_flags =
            spotter_core::analyze(&spotter_core::example::input(settings.today), &settings)
                .flags
                .len();
        let page_ready_ms = self.page_ready_ms.get().copied();
        let peak_rss_mb = metrics::peak_rss_bytes() as f64 / (1024.0 * 1024.0);

        let mut failures = Vec::new();
        if peak_rss_mb > MAX_RSS_MODEL_OFF_MB {
            failures.push(format!(
                "peak RSS {peak_rss_mb:.0} MB > {MAX_RSS_MODEL_OFF_MB} MB with model off"
            ));
        }
        match page_ready_ms {
            Some(ms) if ms <= MAX_PAGE_READY_MS => {}
            Some(ms) => failures.push(format!("page ready after {ms} ms > {MAX_PAGE_READY_MS} ms")),
            None => failures.push("page never asked for its mode".into()),
        }
        if rendered_flags != core_flags {
            failures.push(format!(
                "page shows {rendered_flags} flags, core found {core_flags}"
            ));
        }

        let report = json!({
            "self_test": if failures.is_empty() { "pass" } else { "fail" },
            "version": env!("CARGO_PKG_VERSION"),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "model": "off",
            "page_ready_ms": page_ready_ms,
            "total_ms": started.elapsed().as_millis(),
            "peak_rss_mb": (peak_rss_mb * 10.0).round() / 10.0,
            "flags_core": core_flags,
            "flags_rendered": rendered_flags,
            "failures": failures,
        });
        emit(&report, self.json_out.as_ref());
        if report["self_test"] == "pass" { 0 } else { 1 }
    }
}

fn emit(report: &Value, json_out: Option<&PathBuf>) {
    let line = report.to_string();
    println!("{line}");
    if let Some(path) = json_out {
        if let Err(err) = std::fs::write(path, format!("{line}\n")) {
            eprintln!("could not write {}: {err}", path.display());
        }
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
