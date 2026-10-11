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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Report {
    pub flags: Vec<Flag>,
}
