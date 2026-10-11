//! The owner's settings for one check. Nothing here is persisted between
//! launches; the app saves it inside the "Schedule check" workbook instead.

use std::collections::BTreeSet;

use chrono::{Datelike, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

/// Which weekdays are working days, Monday first. All seven are on by
/// default (Austin's decision, 2026-10-10; see DECISIONS.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workdays(pub [bool; 7]);

impl Default for Workdays {
    fn default() -> Self {
        Workdays([true; 7])
    }
}

impl Workdays {
    pub const MON_FRI: Workdays = Workdays([true, true, true, true, true, false, false]);
    pub const MON_SAT: Workdays = Workdays([true, true, true, true, true, true, false]);
    pub const ALL: Workdays = Workdays([true; 7]);

    pub fn is_workday(&self, weekday: Weekday) -> bool {
        self.0[weekday.num_days_from_monday() as usize]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub workdays: Workdays,
    /// Specific dates that are off for everyone, such as a holiday.
    pub days_off: BTreeSet<NaiveDate>,
    /// Anchors year inference for dates written without a year.
    pub today: NaiveDate,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            workdays: Workdays::default(),
            days_off: BTreeSet::new(),
            today: NaiveDate::from_ymd_opt(2026, 10, 10).expect("valid date"),
        }
    }
}

impl Settings {
    pub fn is_working_date(&self, date: NaiveDate) -> bool {
        self.workdays.is_workday(date.weekday()) && !self.days_off.contains(&date)
    }
}
