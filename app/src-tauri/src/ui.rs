//! What the page receives: the findings, questions and week grid in the
//! words the owner reads, without the bulk the page doesn't need (the
//! assignments themselves stay in Rust for saving).

use chrono::NaiveDate;
use serde::Serialize;
use spotter_core::describe::{self, QuestionView};
use spotter_core::model::{FlagKind, Layout, ReadError};
use spotter_core::session::{EDIT_MAX_ROWS, Session};
use spotter_core::view::WeekView;
use spotter_core::{Answer, Question, Report, export};

#[derive(Clone, Serialize)]
pub struct UiReport {
    /// Bumped on every change; stale model passes are dropped by it.
    pub generation: u64,
    /// "off": no model; "pending": the model is still looking; "done".
    pub smart: &'static str,
    pub status_line: &'static str,
    pub name: String,
    pub error: Option<String>,
    pub notes: Vec<String>,
    pub flags: Vec<UiFlag>,
    pub info: Vec<UiInfo>,
    pub chips: Vec<UiChip>,
    pub questions: Vec<QuestionView>,
    pub answered: Vec<UiAnswered>,
    pub week: WeekView,
    pub reading: UiReading,
    pub workdays: [bool; 7],
    pub days_off: Vec<NaiveDate>,
    pub save_name: String,
    pub editable: bool,
}

#[derive(Clone, Serialize)]
pub struct UiFlag {
    pub kind: FlagKind,
    pub label: &'static str,
    pub date: NaiveDate,
    pub message: String,
    pub person: Option<String>,
    pub site: Option<String>,
    pub sites: Vec<String>,
    pub chips: Vec<usize>,
}

#[derive(Clone, Serialize)]
pub struct UiInfo {
    pub label: &'static str,
    pub date: NaiveDate,
    pub message: String,
}

#[derive(Clone, Serialize)]
pub struct UiChip {
    pub index: usize,
    pub kind: &'static str,
    pub text: String,
    pub reason: &'static str,
}

#[derive(Clone, Serialize)]
pub struct UiAnswered {
    pub subject: String,
    pub text: String,
    pub answer: String,
    /// A "no" that split a chip; offered as "Put back".
    pub undid_chip: bool,
}

#[derive(Clone, Serialize)]
pub struct UiReading {
    pub sheet: String,
    pub layout: Option<String>,
    pub layout_options: Vec<String>,
    pub columns: Vec<UiColumn>,
    /// Column roles can be corrected (a list); grids are corrected by layout.
    pub roles_editable: bool,
    /// The roles a column can be set to.
    pub roles: Vec<&'static str>,
    pub sheets: Vec<String>,
}

#[derive(Clone, Serialize)]
pub struct UiColumn {
    pub header: String,
    pub role: String,
}

/// The roles offered in "What I read" (spec: Person / Job / Date / Day /
/// Time / Status / Ignore, plus the reader's start, end and note).
pub const ROLES: [&str; 10] = [
    "Person",
    "Job",
    "Date",
    "Day",
    "Time",
    "Start time",
    "End time",
    "Status",
    "Note",
    "Ignore",
];

pub fn error_text(e: ReadError) -> &'static str {
    match e {
        ReadError::EmptyFile => "That file is empty.",
        ReadError::NotASchedule => {
            "I couldn't find a schedule in that file: no column or row of dates or weekdays next to names. \
             If it's a picture or a PDF, copy the cells from the spreadsheet instead and paste them here."
        }
        ReadError::Unreadable => {
            "I can't read that file. Save it from your spreadsheet as .xlsx or .csv, or copy the cells and paste them here."
        }
    }
}

pub fn build(session: &Session, report: &Report, generation: u64, smart: &'static str) -> UiReport {
    let flags = report
        .flags
        .iter()
        .map(|f| UiFlag {
            kind: f.kind,
            label: export::flag_label(f.kind),
            date: f.date,
            message: f.message.clone(),
            person: f.person.clone(),
            site: f.site.clone(),
            sites: f.sites.clone(),
            chips: f.merges.clone(),
        })
        .collect();
    let info = report
        .info
        .iter()
        .map(|i| UiInfo {
            label: export::info_label(i.kind),
            date: i.date,
            message: i.message.clone(),
        })
        .collect();
    let chips = report
        .merges
        .iter()
        .enumerate()
        .map(|(index, m)| UiChip {
            index,
            kind: match m.kind {
                spotter_core::matching::MergeKind::Person => "person",
                spotter_core::matching::MergeKind::Site => "site",
            },
            text: describe::merge(m),
            reason: describe::reason(m.reason),
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    let mut answered = Vec::new();
    for (q, a) in session.answers.items.iter().rev() {
        let subject = q.subject();
        if !seen.insert(subject.clone()) {
            continue;
        }
        // "What I read" corrections show in that panel, not here.
        if matches!(q, Question::ColumnRole { options, .. } | Question::PickLayout { options, .. } if options.is_empty())
        {
            continue;
        }
        answered.push(UiAnswered {
            undid_chip: matches!(q, Question::SamePerson { .. } | Question::SameSite { .. })
                && *a == Answer::No
                && !report.questions.iter().any(|x| x.subject() == subject),
            text: describe::question(q).text,
            answer: describe::answer(a),
            subject,
        });
    }
    answered.reverse();
    let r = &report.reading;
    let sheets = report
        .questions
        .iter()
        .find_map(|q| match q {
            Question::PickSheet { options, .. } => Some(options.clone()),
            _ => None,
        })
        .unwrap_or_default();
    // A cheap guess; the table command gives the final word on size.
    let editable = report.error.is_none() && r.assignments.len() <= EDIT_MAX_ROWS * 8;
    UiReport {
        generation,
        smart,
        status_line: crate::model::status_line(),
        name: session.name().to_string(),
        error: report.error.map(|e| error_text(e).to_string()),
        notes: r.notes.clone(),
        flags,
        info,
        chips,
        questions: report.questions.iter().map(describe::question).collect(),
        answered,
        week: report.week.clone(),
        reading: UiReading {
            sheet: r.sheet.clone(),
            layout: r
                .layout
                .map(|l| spotter_core::layout::layout_name(l).to_string()),
            layout_options: r.layout_options.clone(),
            columns: r
                .columns
                .iter()
                .map(|c| UiColumn {
                    header: c.header.clone(),
                    role: c.role.clone(),
                })
                .collect(),
            roles_editable: r.layout == Some(Layout::LongList),
            roles: ROLES.to_vec(),
            sheets,
        },
        workdays: session.settings.workdays.0,
        days_off: session.settings.days_off.iter().copied().collect(),
        save_name: export::default_name(report, &session.settings),
        editable,
    }
}
