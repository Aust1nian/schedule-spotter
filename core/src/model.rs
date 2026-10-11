//! Types shared by every stage and by the app.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// What the owner gave us: a file's bytes, or pasted text.
#[derive(Clone, Debug)]
pub enum Input {
    File { name: String, bytes: Vec<u8> },
    Paste { text: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Off,
    Pto,
    Vacation,
    Sick,
    Holiday,
    Out,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    Am,
    Pm,
    Half,
}

/// Where in the input a value came from (0-based, original coordinates).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct CellRef {
    pub sheet: u16,
    pub row: u32,
    pub col: u32,
}

/// One person (or one unstaffed slot) on one date, as written.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Assignment {
    pub date: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub person: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub unstaffed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<Part>,
    /// Minutes after midnight.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<u16>,
    /// Minutes after midnight; less than `start` means the next day.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub source: CellRef,
}

impl Assignment {
    pub fn new(date: NaiveDate, source: CellRef) -> Assignment {
        Assignment {
            date,
            person: None,
            site: None,
            status: None,
            unstaffed: false,
            part: None,
            start: None,
            end: None,
            note: None,
            source,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    LongList,
    PersonByDay,
    DayByPerson,
    JobByDay,
    DayByJob,
    CrewBased,
    CalendarMonth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arrangement {
    Single,
    Stacked,
    SideBySide,
}

/// A question for the owner. Every question has a default reading already
/// applied, so results never wait on an answer.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Question {
    SamePerson {
        a: String,
        b: String,
    },
    SameSite {
        a: String,
        b: String,
    },
    PickSheet {
        options: Vec<String>,
        chosen: String,
    },
    PickLayout {
        options: Vec<String>,
        chosen: String,
    },
    WeekdayMismatch {
        text: String,
        used: NaiveDate,
    },
    FillDown {
        column: String,
        block_row: u32,
    },
    ColumnRole {
        header: String,
        options: Vec<String>,
        chosen: String,
    },
    CellMeaning {
        text: String,
    },
    PasteAsTable,
}

/// One column (or axis) the reader assigned a role to, for "What I read".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnReading {
    pub index: u32,
    pub header: String,
    pub role: String,
}

/// How the schedule was read: the "What I read" panel and the input to
/// analysis.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Reading {
    pub sheet: String,
    pub layout: Option<Layout>,
    pub arrangement: Option<Arrangement>,
    pub columns: Vec<ColumnReading>,
    pub assignments: Vec<Assignment>,
    /// Every date the schedule covers (see FORMAT.md "Window").
    pub window: Vec<NaiveDate>,
    /// Job rows/columns of a job grid: those jobs are active on every
    /// working date of the window except the `inactive` cells.
    pub grid_jobs: Vec<String>,
    pub inactive: Vec<(String, NaiveDate)>,
    /// Names from a roster block, when the file has one.
    pub roster: Option<Vec<String>>,
    pub questions: Vec<Question>,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlagKind {
    DoubleBooked,
    AssignedWhileOff,
    NotBooked,
    Unstaffed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flag {
    pub kind: FlagKind,
    pub person: Option<String>,
    pub job: Option<String>,
    pub date: NaiveDate,
    pub sites: Vec<String>,
    pub message: String,
}

/// Why a file couldn't be read at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadError {
    EmptyFile,
    NotASchedule,
    Unreadable,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Report {
    pub reading: Reading,
    pub error: Option<ReadError>,
    pub flags: Vec<Flag>,
}
