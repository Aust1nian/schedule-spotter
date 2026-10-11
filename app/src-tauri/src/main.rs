// Release builds on Windows have no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod metrics;
mod selftest;

use std::time::Instant;

use serde::Serialize;
use spotter_core::{Report, Settings};
use tauri::{AppHandle, State};

struct AppState {
    started: Instant,
    self_test: Option<selftest::SelfTest>,
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
        app.exit(code);
    }
}

fn main() {
    let started = Instant::now();
    let args = selftest::Args::parse(std::env::args().skip(1));
    let self_test = args.self_test.then(|| {
        selftest::attach_parent_console();
        selftest::SelfTest::start(args.json_out.clone())
    });

    tauri::Builder::default()
        .manage(AppState { started, self_test })
        .invoke_handler(tauri::generate_handler![
            app_mode,
            run_example,
            self_test_done
        ])
        .run(tauri::generate_context!())
        .expect("Schedule Spotter failed to start");
}
