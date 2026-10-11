//! Schedule Spotter core: everything between the bytes of a schedule file and
//! the findings shown to the owner. No UI and no model code lives here; the
//! model is reached only through the tiebreaker interface added in Phase 4.

pub mod example;
pub mod model;
pub mod settings;

pub use model::{Flag, FlagKind, Input, Report};
pub use settings::{Settings, Workdays};

/// Reads `input` and computes the findings under `settings`.
///
/// Phase 0 scaffold: returns an empty report. Stages 1–10 land in Phases 1–3.
pub fn analyze(input: &Input, settings: &Settings) -> Report {
    let _ = (input, settings);
    Report::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_runs() {
        let settings = Settings::default();
        let report = analyze(&example::input(settings.today), &settings);
        assert!(report.flags.len() <= 64);
    }
}
