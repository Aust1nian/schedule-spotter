//! The model, loaded on a background thread after the window shows (spec:
//! "Loading"). Until it's ready, and for good if it can't load, every
//! uncertain item goes to the owner.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use spotter_core::{NoModel, Tiebreaker};
use spotter_llm::{Engine, Task};

pub const VARIANT: &str = env!("SPOTTER_VARIANT");
pub const FILE: &str = env!("SPOTTER_MODEL_FILE");
pub const SHA256: &str = env!("SPOTTER_MODEL_SHA");
pub const NAME: &str = env!("SPOTTER_MODEL_NAME");
pub const LICENSE: &str = env!("SPOTTER_MODEL_LICENSE");

static ENGINE: OnceLock<Engine> = OnceLock::new();
static STATUS: Mutex<Status> = Mutex::new(Status {
    state: State::Loading,
    detail: String::new(),
    load_secs: None,
    backend: None,
});

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Loading,
    Ready,
    Off,
}

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub state: State,
    /// Why it's off, for About and diagnostics.
    pub detail: String,
    pub load_secs: Option<f64>,
    pub backend: Option<&'static str>,
}

pub fn status() -> Status {
    STATUS.lock().map(|s| s.clone()).unwrap_or(Status {
        state: State::Off,
        detail: String::new(),
        load_secs: None,
        backend: None,
    })
}

fn set(status: Status) {
    if let Ok(mut s) = STATUS.lock() {
        *s = status;
    }
}

/// The status line under the toolbar. A build without a model (the shipped
/// choice, MODEL_REPORT.md) says how it works rather than that something is
/// off.
pub fn status_line() -> &'static str {
    if VARIANT == "off" {
        return "I'll ask you about any name or site I'm unsure of";
    }
    match status().state {
        State::Loading => {
            "Smart matching is starting\u{2026} I'll ask you about anything unsure meanwhile"
        }
        State::Ready => spotter_llm::status_line(true),
        State::Off => spotter_llm::status_line(false),
    }
}

/// What answers tiebreaker questions right now.
pub fn tiebreak() -> &'static dyn Tiebreaker {
    match ENGINE.get() {
        Some(e) => e,
        None => &NoModel,
    }
}

pub fn ready() -> bool {
    ENGINE.get().is_some()
}

/// llama.cpp calls that failed since the model loaded.
pub fn failures() -> usize {
    ENGINE.get().map_or(0, |e| e.failures())
}

/// Where the bundled model lives: `<resources>/model/<file>`. Debug builds
/// also look in the repo's `models/` folder.
fn find(resources: Option<&Path>) -> Option<PathBuf> {
    let mut places: Vec<PathBuf> = Vec::new();
    if let Some(r) = resources {
        places.push(r.join("model").join(FILE));
    }
    if cfg!(debug_assertions) {
        places.push(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../models")
                .join(FILE),
        );
    }
    places.into_iter().find(|p| p.is_file())
}

/// Loads the model, then calls `on_change`. Runs on its own thread.
pub fn load(resources: Option<PathBuf>, on_change: impl Fn() + Send + 'static) {
    if VARIANT == "off" {
        set(Status {
            state: State::Off,
            detail: "this build has no model".into(),
            load_secs: None,
            backend: None,
        });
        on_change();
        return;
    }
    std::thread::Builder::new()
        .name("model-load".into())
        .spawn(move || {
            let Some(path) = find(resources.as_deref()) else {
                set(Status {
                    state: State::Off,
                    detail: "the model file is missing".into(),
                    load_secs: None,
                    backend: None,
                });
                on_change();
                return;
            };
            match Engine::load(&path, Some(SHA256), VARIANT, spotter_llm::default_threads()) {
                Ok(engine) => {
                    // The tasks a file needs first: reading, then matching.
                    engine.warm(&[
                        Task::ColumnRole,
                        Task::CellMeaning,
                        Task::SamePerson,
                        Task::SameSite,
                    ]);
                    let (secs, backend) = (engine.load_secs, engine.backend);
                    let _ = ENGINE.set(engine);
                    set(Status {
                        state: State::Ready,
                        detail: String::new(),
                        load_secs: Some(secs),
                        backend: Some(backend),
                    });
                }
                Err(e) => set(Status {
                    state: State::Off,
                    detail: e.to_string(),
                    load_secs: None,
                    backend: None,
                }),
            }
            on_change();
        })
        .expect("model thread");
}
