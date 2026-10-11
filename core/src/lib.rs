//! Schedule Spotter core: everything between the bytes of a schedule file and
//! the findings shown to the owner. No UI and no model code lives here; the
//! model is reached only through the tiebreaker interface added in Phase 4.

pub mod cells;
pub mod classify;
pub mod dates;
pub mod example;
pub mod grid;
pub mod layout;
mod layout_calendar;
mod layout_grid;
mod layout_list;
pub mod lexicon;
pub mod model;
pub mod read;
mod read_docx;
mod read_numbers;
pub mod settings;
pub mod text;
pub mod times;

pub use model::{Flag, FlagKind, Input, Question, ReadError, Reading, Report};
pub use settings::{Settings, Workdays};

/// Stages 1–6: reads `input` into assignments, or says why it can't.
pub fn read_schedule(input: &Input, settings: &Settings) -> Result<Reading, ReadError> {
    let workbook = read::read(input)?;
    let mut reading = layout::read_workbook(&workbook, settings);
    if reading.assignments.is_empty() && reading.grid_jobs.is_empty() {
        if matches!(input, Input::Paste { .. }) {
            // Free-form text needs the model (task T5); without it, ask for a table.
            reading.questions.push(Question::PasteAsTable);
            return Ok(reading);
        }
        return Err(ReadError::NotASchedule);
    }
    Ok(reading)
}

/// Reads `input` and computes the findings under `settings`.
pub fn analyze(input: &Input, settings: &Settings) -> Report {
    match read_schedule(input, settings) {
        Ok(reading) => Report {
            reading,
            error: None,
            flags: Vec::new(),
        },
        Err(error) => Report {
            reading: Reading::default(),
            error: Some(error),
            flags: Vec::new(),
        },
    }
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
