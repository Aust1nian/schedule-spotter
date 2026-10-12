//! Plain-English wording for questions and merges, shared by the app and
//! the saved workbook so both say the same thing.

use serde::{Deserialize, Serialize};

use crate::matching::{Merge, MergeKind, MergeReason};
use crate::model::{Answer, Question};

/// A question as the owner sees it: the sentence, and the buttons.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestionView {
    pub subject: String,
    pub text: String,
    /// Button labels. "Yes" / "No" for yes-no questions; the choices for
    /// pick questions; empty when there is nothing to answer.
    pub options: Vec<String>,
    /// The reading in use until the owner answers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    pub question: Question,
}

impl QuestionView {
    /// The answer a button means.
    pub fn answer_for(&self, option: &str) -> Answer {
        match (&self.question, option) {
            (
                Question::PickSheet { .. }
                | Question::PickLayout { .. }
                | Question::ColumnRole { .. },
                o,
            ) => Answer::Pick(o.to_string()),
            (_, "Yes") => Answer::Yes,
            _ => Answer::No,
        }
    }
}

fn yes_no() -> Vec<String> {
    vec!["Yes".into(), "No".into()]
}

pub fn question(q: &Question) -> QuestionView {
    let (text, options, current) = match q {
        Question::SamePerson { a, b } => (
            format!("Are \u{201c}{a}\u{201d} and \u{201c}{b}\u{201d} the same person?"),
            yes_no(),
            None,
        ),
        Question::SameSite { a, b } => (
            format!("Are \u{201c}{a}\u{201d} and \u{201c}{b}\u{201d} the same job?"),
            yes_no(),
            None,
        ),
        Question::PickSheet { options, chosen } => (
            "This workbook has more than one sheet that looks like a schedule. Which one should I check?"
                .to_string(),
            options.clone(),
            Some(chosen.clone()),
        ),
        Question::PickLayout { options, chosen } => (
            "I can read this schedule two ways. Which is right?".to_string(),
            options.clone(),
            Some(chosen.clone()),
        ),
        Question::WeekdayMismatch { text, used } => (
            format!(
                "\u{201c}{text}\u{201d} names a different weekday than its date: {} is a {}. Is the date right?",
                used.format("%-m/%-d/%Y"),
                used.format("%A")
            ),
            yes_no(),
            None,
        ),
        Question::FillDown { column, .. } => (
            format!(
                "Some cells under \u{201c}{column}\u{201d} are blank. Does each blank mean the same as the cell above it?"
            ),
            yes_no(),
            None,
        ),
        Question::ColumnRole {
            header,
            options,
            chosen,
        } => (
            format!("Is \u{201c}{header}\u{201d} a list of people or of jobs?"),
            options.clone(),
            Some(chosen.clone()),
        ),
        Question::CellMeaning { text } => (
            format!("I couldn't tell what \u{201c}{text}\u{201d} means, so I left it out."),
            Vec::new(),
            None,
        ),
        Question::PasteAsTable => (
            "I can't read free-form text with smart matching off. Copy the cells from your spreadsheet and paste them instead."
                .to_string(),
            Vec::new(),
            None,
        ),
    };
    QuestionView {
        subject: q.subject(),
        text,
        options,
        current,
        question: q.clone(),
    }
}

/// "Treating Mike B as Michael Baker".
pub fn merge(m: &Merge) -> String {
    format!("Treating {} as {}", m.from, m.into)
}

pub fn reason(r: MergeReason) -> &'static str {
    match r {
        MergeReason::Spelling => "same name, different spelling",
        MergeReason::Nickname => "nickname",
        MergeReason::OnlyMatch => "the only name it fits",
        MergeReason::Address => "same address, different abbreviation",
        MergeReason::JobNumber => "same job with its job number",
        MergeReason::Model => "smart matching",
        MergeReason::Answered => "you said so",
    }
}

/// The question whose "No" undoes a merge.
pub fn undo_question(m: &Merge) -> Question {
    match m.kind {
        MergeKind::Person => Question::SamePerson {
            a: m.from.clone(),
            b: m.into.clone(),
        },
        MergeKind::Site => Question::SameSite {
            a: m.from.clone(),
            b: m.into.clone(),
        },
    }
}

/// An answer as text, for the saved workbook.
pub fn answer(a: &Answer) -> String {
    match a {
        Answer::Yes => "Yes".into(),
        Answer::No => "No".into(),
        Answer::Pick(o) => o.clone(),
    }
}
