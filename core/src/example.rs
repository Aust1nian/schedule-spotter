//! The built-in example week the app opens with: a person × day grid for
//! the current week (next week's on a weekend, when owners lay it out),
//! with one of each finding, a "treating A as B" chip and a question, so
//! the owner sees what the tool does before opening anything.

use chrono::{Datelike, Duration, NaiveDate, Weekday};

use crate::model::Input;

/// The Monday of the week the example shows.
pub fn monday(today: NaiveDate) -> NaiveDate {
    let back = today.weekday().num_days_from_monday() as i64;
    let monday = today - Duration::days(back);
    if matches!(today.weekday(), Weekday::Sat | Weekday::Sun) {
        monday + Duration::days(7)
    } else {
        monday
    }
}

/// The example as tab-separated text, the way it would be pasted.
pub fn text(today: NaiveDate) -> String {
    let mon = monday(today);
    let mut header = vec!["Crew".to_string()];
    for i in 0..5 {
        let d = mon + Duration::days(i);
        header.push(d.format("%a %-m/%-d").to_string());
    }
    // Mon, Tue, Wed, Thu, Fri
    let rows: [[&str; 6]; 8] = [
        [
            "Michael Baker",
            "1420 Johnson St",
            "1420 Johnson St",
            "1420 Johnson St / Smith remodel",
            "Smith remodel",
            "1420 Johnson Street",
        ],
        ["Mike B", "", "", "", "Parker kitchen", ""],
        [
            "José García",
            "Oak Ridge Apts",
            "Oak Ridge Apts",
            "PTO",
            "PTO / Oak Ridge Apts",
            "Oak Ridge Apts",
        ],
        [
            "Luis Ortiz",
            "Smith remodel",
            "Smith remodel",
            "Smith remodel",
            "",
            "Smith remodel",
        ],
        [
            "Dave Kim",
            "Parker kitchen",
            "Parker kitchen",
            "Parker kitchen",
            "Parker kitchen",
            "Parker kitchen",
        ],
        [
            "Ana Ruiz",
            "Oak Ridge Apts",
            "Oak Ridge Apts",
            "Shop",
            "Oak Ridge Apts",
            "Oak Ridge Apts",
        ],
        [
            "Sam Patel",
            "1420 Johnson St",
            "Johnson job",
            "1420 Johnson St",
            "1420 Johnson St",
            "1420 Johnson St",
        ],
        ["Helper 1", "1420 Johnson St", "1420 Johnson St", "", "", ""],
    ];
    let mut out = header.join("\t");
    for row in rows {
        out.push('\n');
        out.push_str(&row.join("\t"));
    }
    out.push('\n');
    out
}

/// The example schedule, as if pasted.
pub fn input(today: NaiveDate) -> Input {
    Input::Paste { text: text(today) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekend_shows_next_week() {
        let sat = NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();
        let wed = NaiveDate::from_ymd_opt(2026, 10, 14).unwrap();
        assert_eq!(monday(sat), NaiveDate::from_ymd_opt(2026, 10, 12).unwrap());
        assert_eq!(monday(wed), NaiveDate::from_ymd_opt(2026, 10, 12).unwrap());
    }
}
