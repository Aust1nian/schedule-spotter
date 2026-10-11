//! Schedule Spotter core: everything between the bytes of a schedule file and
//! the findings shown to the owner. No UI and no model code lives here; the
//! model is reached only through the tiebreaker interface added in Phase 4.

pub mod analyze;
pub mod cells;
pub mod classify;
pub mod dates;
pub mod example;
pub mod grid;
pub mod identity;
pub mod layout;
mod layout_calendar;
mod layout_grid;
mod layout_list;
pub mod lexicon;
pub mod matching;
pub mod model;
pub mod read;
mod read_docx;
mod read_numbers;
pub mod settings;
pub mod text;
pub mod times;

pub use matching::{NoModel, Tiebreaker};
pub use model::{Answer, Answers, Flag, FlagKind, Input, Question, ReadError, Reading, Report};
pub use settings::{Settings, Workdays};

/// Stages 1–6: reads `input` into assignments, or says why it can't.
pub fn read_schedule(input: &Input, settings: &Settings) -> Result<Reading, ReadError> {
    read_schedule_with(input, settings, &Answers::default())
}

/// Stages 1–6 with the owner's answers about sheets, layouts, dates,
/// fill-downs and column roles applied.
pub fn read_schedule_with(
    input: &Input,
    settings: &Settings,
    answers: &Answers,
) -> Result<Reading, ReadError> {
    let workbook = read::read(input)?;
    let mut reading = layout::read_workbook(&workbook, settings, answers);
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

/// Reads `input` and computes the findings under `settings`, with no
/// answers yet and the model off.
pub fn analyze(input: &Input, settings: &Settings) -> Report {
    analyze_with(input, settings, &Answers::default(), &NoModel)
}

/// The whole pipeline: read (stages 1–6), match (7), classify and flag
/// (8–9), honoring the owner's answers and asking the model about
/// anything the rules can't settle.
pub fn analyze_with(
    input: &Input,
    settings: &Settings,
    answers: &Answers,
    tiebreak: &dyn Tiebreaker,
) -> Report {
    match read_schedule_with(input, settings, answers) {
        Ok(reading) => {
            let matching = matching::match_entities(&reading, answers, tiebreak);
            let (flags, info) = analyze::analyze(&reading, settings, &matching);
            let mut questions = reading.questions.clone();
            questions.extend(matching.questions.iter().cloned());
            Report {
                reading,
                error: None,
                flags,
                info,
                merges: matching.merges,
                questions,
            }
        }
        Err(error) => Report {
            error: Some(error),
            ..Report::default()
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
