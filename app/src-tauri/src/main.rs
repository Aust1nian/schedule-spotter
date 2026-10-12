// Release builds on Windows have no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod metrics;
mod model;
mod payload;
mod scratch;
mod selftest;
mod ui;

use std::time::Instant;

use commands::AppState;
use tauri::{
    DragDropEvent, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

/// WebView2 runs Edge's background services unless told not to: component
/// updates, SmartScreen, phishing checks, sync, pings, crash upload. All
/// off, and no host name resolves at all, not even the app's own
/// tauri.localhost: Tauri answers those requests in-process, and without
/// the rule Chromium pre-connects to loopback port 80 anyway. The CI
/// no-socket test checks this.
/// (wry's own defaults, msWebOOUI, msPdfOOUI and msSmartScreenProtection, are
/// repeated here because setting arguments replaces them.)
#[cfg_attr(feature = "ui-test", allow(dead_code))]
const WEBVIEW2_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,msExperimentation,msEdgeUpdate,AutofillServerCommunication,OptimizationHints,Translate,MediaRouter \
--disable-background-networking --disable-component-update --disable-domain-reliability \
--disable-client-side-phishing-detection --disable-sync --disable-default-apps --no-pings --no-first-run \
--disable-breakpad --dns-prefetch-disable --host-resolver-rules=\"MAP * ~NOTFOUND\"";

fn main() {
    let started = Instant::now();
    // A hidden copy of the app that removes the last session's folder once
    // WebView2 lets go of it (scratch::remove).
    let raw: Vec<String> = std::env::args().collect();
    if raw.len() == 4 && raw[1] == "--remove-scratch" {
        let pid = raw[3].parse().unwrap_or(0);
        std::process::exit(scratch::remove_when_free(
            std::path::Path::new(&raw[2]),
            pid,
        ));
    }
    let args = selftest::Args::parse(std::env::args().skip(1));
    // Run by the Windows installer only: unpack the model it carries.
    if let Some(setup) = &args.install_model {
        std::process::exit(payload::install_from_args(setup));
    }
    let self_test = args.self_test.then(|| {
        selftest::attach_parent_console();
        selftest::SelfTest::start(args.json_out.clone(), args.linger)
    });
    let is_self_test = self_test.is_some();
    let scratch = scratch::create().expect("Schedule Spotter needs its temp folder");

    let builder =
        tauri::Builder::default().manage(AppState::new(started, self_test, scratch.clone()));
    #[cfg(not(feature = "ui-test"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::app_mode,
        commands::start,
        commands::example,
        commands::answer,
        commands::undo_chip,
        commands::forget_answer,
        commands::set_role,
        commands::set_layout,
        commands::set_sheet,
        commands::set_workdays,
        commands::toggle_day_off,
        commands::table,
        commands::set_table,
        commands::paste,
        commands::open_file,
        commands::save,
        commands::diagnostics,
        commands::save_diagnostics,
        commands::about,
        commands::self_test_done,
        commands::page_error,
    ]);
    #[cfg(feature = "ui-test")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::app_mode,
        commands::start,
        commands::example,
        commands::answer,
        commands::undo_chip,
        commands::forget_answer,
        commands::set_role,
        commands::set_layout,
        commands::set_sheet,
        commands::set_workdays,
        commands::toggle_day_off,
        commands::table,
        commands::set_table,
        commands::paste,
        commands::open_file,
        commands::save,
        commands::diagnostics,
        commands::save_diagnostics,
        commands::about,
        commands::self_test_done,
        commands::page_error,
        commands::test_open,
        commands::test_save,
    ]);

    let app = builder
        .on_window_event(|window, event| {
            let WindowEvent::DragDrop(drop) = event else {
                return;
            };
            let app = window.app_handle().clone();
            match drop {
                DragDropEvent::Enter { .. } | DragDropEvent::Over { .. } => {
                    let _ = app.emit("drag", true);
                }
                DragDropEvent::Leave => {
                    let _ = app.emit("drag", false);
                }
                DragDropEvent::Drop { paths, .. } => {
                    let _ = app.emit("drag", false);
                    let Some(path) = paths.first().cloned() else {
                        return;
                    };
                    std::thread::spawn(move || match commands::open_path(&app, &path) {
                        Ok(report) => {
                            let _ = app.emit("report", report);
                        }
                        Err(message) => {
                            let _ = app.emit("problem", message);
                        }
                    });
                }
                _ => {}
            }
        })
        .setup(move |app| {
            let window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("Schedule Spotter")
                    .inner_size(1280.0, 800.0)
                    .min_inner_size(960.0, 640.0);
            // UI-test builds leave the webview to WebDriver: WebKitGTK
            // automation can't attach to an incognito webview's own
            // ephemeral context, and Edge WebDriver sets WebView2's data
            // folder and debugging port through environment variables that
            // these settings would override.
            #[cfg(not(feature = "ui-test"))]
            let window = window
                .incognito(true)
                .data_directory(scratch.join("webview"))
                .additional_browser_args(WEBVIEW2_ARGS);
            // Windows UI tests attach Edge WebDriver to this port
            // (`debuggerAddress`) instead of having it launch the app.
            #[cfg(feature = "ui-test")]
            let window = match std::env::var("SPOTTER_CDP_PORT") {
                Ok(port) => window.additional_browser_args(&format!(
                    "--remote-debugging-port={port} --remote-allow-origins=*"
                )),
                Err(_) => window,
            };
            #[cfg(feature = "ui-test")]
            let _ = &scratch;
            window.build()?;
            // The model loads after the window shows; the self-test loads
            // it once the page has reported (commands::self_test_done).
            let handle = app.handle().clone();
            let resources = app.path().resource_dir().ok();
            if is_self_test {
                return Ok(());
            }
            model::load(resources, move || {
                let state = handle.state::<AppState>();
                state.note(format!("model: {:?}", model::status().state));
                let _ = handle.emit("model", model::status_line());
                let has_session = state.session.lock().map(|s| s.is_some()).unwrap_or(false);
                if has_session {
                    let report = commands::refresh(&handle);
                    let _ = handle.emit("report", report);
                }
            });
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
