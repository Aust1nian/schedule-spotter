// Release builds on Windows have no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod metrics;
mod scratch;
mod selftest;

use std::path::PathBuf;
use std::time::Instant;

use serde::Serialize;
use spotter_core::{Report, Settings};
use tauri::{AppHandle, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder};

/// WebView2 runs Edge's background services unless told not to: component
/// updates, SmartScreen, phishing checks, sync, pings, crash upload. All
/// off, and no host name resolves at all, not even the app's own
/// tauri.localhost: Tauri answers those requests in-process, and without
/// the rule Chromium pre-connects to loopback port 80 anyway. The CI
/// no-socket test checks this.
/// (wry's own defaults, msWebOOUI, msPdfOOUI and msSmartScreenProtection, are
/// repeated here because setting arguments replaces them.)
const WEBVIEW2_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,msExperimentation,msEdgeUpdate,AutofillServerCommunication,OptimizationHints,Translate,MediaRouter \
--disable-background-networking --disable-component-update --disable-domain-reliability \
--disable-client-side-phishing-detection --disable-sync --disable-default-apps --no-pings --no-first-run \
--disable-breakpad --dns-prefetch-disable --host-resolver-rules=\"MAP * ~NOTFOUND\"";

struct AppState {
    started: Instant,
    self_test: Option<selftest::SelfTest>,
    scratch: PathBuf,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Mode {
    self_test: bool,
    version: &'static str,
    model_status: &'static str,
}

#[tauri::command]
fn app_mode(state: State<'_, AppState>) -> Mode {
    if let Some(test) = &state.self_test {
        test.mark_page_ready(state.started);
    }
    Mode {
        self_test: state.self_test.is_some(),
        version: env!("CARGO_PKG_VERSION"),
        model_status: spotter_llm::status_line(false),
    }
}

#[tauri::command]
fn run_example() -> Report {
    let settings = Settings::default();
    spotter_core::analyze(&spotter_core::example::input(settings.today), &settings)
}

#[tauri::command]
fn self_test_done(app: AppHandle, state: State<'_, AppState>, rendered_flags: usize) {
    if let Some(test) = &state.self_test {
        let code = test.finish(state.started, rendered_flags);
        let linger = test.linger();
        // Stay open a while so CI can watch for late network activity.
        std::thread::spawn(move || {
            std::thread::sleep(linger);
            app.exit(code);
        });
    }
}

fn main() {
    let started = Instant::now();
    let args = selftest::Args::parse(std::env::args().skip(1));
    let self_test = args.self_test.then(|| {
        selftest::attach_parent_console();
        selftest::SelfTest::start(args.json_out.clone(), args.linger)
    });
    let scratch = scratch::create().expect("Schedule Spotter needs its temp folder");

    let app = tauri::Builder::default()
        .manage(AppState {
            started,
            self_test,
            scratch: scratch.clone(),
        })
        .invoke_handler(tauri::generate_handler![
            app_mode,
            run_example,
            self_test_done
        ])
        .setup(move |app| {
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("Schedule Spotter")
                .inner_size(1280.0, 800.0)
                .min_inner_size(960.0, 640.0)
                .incognito(true)
                .data_directory(scratch.join("webview"))
                .additional_browser_args(WEBVIEW2_ARGS)
                .build()?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Schedule Spotter failed to start");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            scratch::remove(&handle.state::<AppState>().scratch);
        }
    });
}
