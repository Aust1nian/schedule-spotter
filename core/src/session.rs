//! One check, as the owner works on it: the schedule they gave, every
//! answer and correction, and their working days. The app holds one; the
//! saved workbook carries it so reopening restores everything; the
//! category N fixtures drive it with the same calls the app makes.

use base64::Engine as _;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::matching::{Merge, NoModel, Tiebreaker};
use crate::model::{Answer, Answers, Input, Question, Report};
use crate::settings::{Settings, Workdays};

/// Where the schedule came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    File {
        name: String,
        #[serde(with = "b64")]
        bytes: Vec<u8>,
    },
    Paste {
        text: String,
    },
    /// The built-in example, or a table the owner edited in the app.
    Table {
        name: String,
        text: String,
    },
}

mod b64 {
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        base64::engine::general_purpose::STANDARD
            .decode(text)
            .map_err(serde::de::Error::custom)
    }
}

/// Editing in the app is offered up to this size; bigger schedules are
/// edited in their own spreadsheet program.
pub const EDIT_MAX_ROWS: usize = 400;
pub const EDIT_MAX_COLS: usize = 60;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    /// Bumped when the saved form changes shape.
    pub version: u32,
    pub source: Source,
    pub answers: Answers,
    pub settings: Settings,
}

impl Session {
    pub const VERSION: u32 = 1;

    pub fn new(source: Source, settings: Settings) -> Session {
        Session {
            version: Session::VERSION,
            source,
            answers: Answers::default(),
            settings,
        }
    }

    /// The built-in example week.
    pub fn example(settings: Settings) -> Session {
        let text = crate::example::text(settings.today);
        Session::new(
            Source::Table {
                name: "Example week".into(),
                text,
            },
            settings,
        )
    }

    /// What the owner would call this schedule.
    pub fn name(&self) -> &str {
        match &self.source {
            Source::File { name, .. } | Source::Table { name, .. } => name,
            Source::Paste { .. } => "Pasted schedule",
        }
    }

    pub fn input(&self) -> Input {
        match &self.source {
            Source::File { name, bytes } => Input::File {
                name: name.clone(),
                bytes: bytes.clone(),
            },
            Source::Paste { text } | Source::Table { text, .. } => {
                Input::Paste { text: text.clone() }
            }
        }
    }

    pub fn report(&self, tiebreak: &dyn Tiebreaker) -> Report {
        crate::analyze_with(&self.input(), &self.settings, &self.answers, tiebreak)
    }

    pub fn report_model_off(&self) -> Report {
        self.report(&NoModel)
    }

    pub fn answer(&mut self, question: Question, answer: Answer) {
        self.answers.items.push((question, answer));
    }

    /// One-tap undo on a "treating A as B" chip.
    pub fn undo_merge(&mut self, merge: &Merge) {
        self.answer(crate::describe::undo_question(merge), Answer::No);
    }

    /// The owner says what a column is ("What I read").
    pub fn set_column_role(&mut self, header: &str, role: &str) {
        self.answer(
            Question::ColumnRole {
                header: header.to_string(),
                options: Vec::new(),
                chosen: role.to_string(),
            },
            Answer::Pick(role.to_string()),
        );
    }

    /// The owner picks how the sheet is laid out ("What I read").
    pub fn set_layout(&mut self, layout: &str) {
        self.answer(
            Question::PickLayout {
                options: Vec::new(),
                chosen: layout.to_string(),
            },
            Answer::Pick(layout.to_string()),
        );
    }

    pub fn set_sheet(&mut self, sheet: &str) {
        self.answer(
            Question::PickSheet {
                options: Vec::new(),
                chosen: sheet.to_string(),
            },
            Answer::Pick(sheet.to_string()),
        );
    }

    pub fn set_workdays(&mut self, workdays: Workdays) {
        self.settings.workdays = workdays;
    }

    /// Turns "day off for everyone" on or off for one date.
    pub fn toggle_day_off(&mut self, date: NaiveDate) {
        if !self.settings.days_off.remove(&date) {
            self.settings.days_off.insert(date);
        }
    }

    /// The sheet being checked as rows of cell text, for editing in the
    /// app; `None` when it's too big to edit there or can't be read.
    pub fn table(&self, sheet: &str) -> Option<Vec<Vec<String>>> {
        let wb = crate::read::read(&self.input()).ok()?;
        let s = wb
            .sheets
            .iter()
            .find(|s| s.name == sheet)
            .or(wb.sheets.first())?;
        let width = s.width();
        if s.rows.len() > EDIT_MAX_ROWS || width > EDIT_MAX_COLS {
            return None;
        }
        Some(
            s.rows
                .iter()
                .map(|r| {
                    (0..width)
                        .map(|c| r.get(c).map(|cell| cell.text.clone()).unwrap_or_default())
                        .collect()
                })
                .collect(),
        )
    }

    /// Replaces the schedule with the owner's edited table. Answers stay:
    /// they are about names and columns, which edits mostly keep.
    pub fn set_table(&mut self, rows: &[Vec<String>]) {
        let name = match &self.source {
            Source::Table { name, .. } => name.clone(),
            Source::File { name, .. } => format!("{name} (edited)"),
            Source::Paste { .. } => "Pasted schedule (edited)".into(),
        };
        self.source = Source::Table {
            name,
            text: to_tsv(rows),
        };
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("session serializes")
    }

    pub fn from_json(text: &str) -> Option<Session> {
        let s: Session = serde_json::from_str(text).ok()?;
        (s.version <= Session::VERSION).then_some(s)
    }

    /// The source's bytes, for a size check before saving.
    pub fn source_len(&self) -> usize {
        match &self.source {
            Source::File { bytes, .. } => base64::engine::general_purpose::STANDARD
                .encode(bytes)
                .len(),
            Source::Paste { text } | Source::Table { text, .. } => text.len(),
        }
    }
}

/// Rows as tab-separated text, quoting cells that hold tabs, line breaks
/// or quotes, the way spreadsheets copy them.
pub fn to_tsv(rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .map(|c| {
                if c.contains(['\t', '\n', '\r', '"']) {
                    format!("\"{}\"", c.replace('"', "\"\""))
                } else {
                    c.clone()
                }
            })
            .collect();
        out.push_str(&cells.join("\t"));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut s = Session::example(Settings::default());
        s.answer(
            Question::SamePerson {
                a: "Mike B".into(),
                b: "Michael Baker".into(),
            },
            Answer::No,
        );
        s.toggle_day_off(NaiveDate::from_ymd_opt(2026, 10, 14).unwrap());
        let back = Session::from_json(&s.to_json()).unwrap();
        assert_eq!(back.source, s.source);
        assert_eq!(back.settings, s.settings);
        assert_eq!(back.answers.items, s.answers.items);
    }

    #[test]
    fn edited_table_reads_the_same() {
        let s = Session::example(Settings::default());
        let before = s.report_model_off();
        let table = s.table("Pasted").unwrap();
        let mut edited = s.clone();
        edited.set_table(&table);
        let after = edited.report_model_off();
        assert_eq!(before.flags, after.flags);
    }

    #[test]
    fn undo_merge_removes_the_chip() {
        let mut s = Session::example(Settings::default());
        let report = s.report_model_off();
        let chip = report
            .merges
            .iter()
            .find(|m| m.from == "Mike B")
            .cloned()
            .expect("example merges Mike B");
        s.undo_merge(&chip);
        let after = s.report_model_off();
        assert!(after.merges.iter().all(|m| m.from != "Mike B"));
        // Michael Baker's Thursday double-booking came from that merge.
        let thursday = |r: &Report| {
            r.flags.iter().any(|f| {
                f.kind == crate::model::FlagKind::DoubleBooked
                    && f.sites.iter().any(|s| s == "Parker kitchen")
            })
        };
        assert!(thursday(&report));
        assert!(!thursday(&after));
    }
}
