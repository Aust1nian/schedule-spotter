//! Model layer: llama.cpp in process, used only as a tiebreaker on closed
//! questions the script layer hands it. Phase 0 scaffold: no model.

/// Status line shown in the app.
pub fn status_line(loaded: bool) -> &'static str {
    if loaded {
        "Smart matching ready"
    } else {
        "Smart matching off, I'll ask you instead"
    }
}
