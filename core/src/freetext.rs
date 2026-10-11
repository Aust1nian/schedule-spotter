//! Free-form pasted text ("Mon: Mike & Jose @ Johnson"): the paste path
//! only, and only with a model (task T5). Each line goes to the model on its
//! own; every person and site it returns must appear in that line, or the
//! line is rejected. With no model, the owner is asked for a table instead.

use std::collections::BTreeSet;

use chrono::{Duration, NaiveDate, Weekday};

use crate::dates;
use crate::matching::Tiebreaker;
use crate::model::{Assignment, CellRef, Question, Reading};
use crate::settings::Settings;
use crate::text::{clean, key};
use crate::times;

fn weekday(day: &str) -> Option<Weekday> {
    dates::weekday_word(day)
}

/// "Mon: Mike & Jose @ Johnson. Tue: Mike - Smith job" is two lines run
/// together: split where a sentence (after ". " or "; ") starts with a
/// weekday, so the model sees one day's line at a time.
fn pieces(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut rest = line;
    while let Some(i) = rest.find(['.', ';']) {
        let (head, tail) = rest.split_at(i);
        let after = &tail[1..];
        let next_word = after
            .trim_start()
            .split(|c: char| !c.is_alphabetic())
            .next()
            .unwrap_or("")
            .to_lowercase();
        current.push_str(head);
        if after.starts_with(' ') && weekday(&next_word).is_some() {
            out.push(std::mem::take(&mut current));
        } else {
            current.push_str(&tail[..1]);
        }
        rest = after;
    }
    current.push_str(rest);
    out.push(current);
    out.into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// The T5 guard: a name or site the model returns must be in the line.
fn in_line(line_key: &str, value: &str) -> bool {
    let v = key(value);
    !v.is_empty() && line_key.contains(&v)
}

pub fn read(text: &str, settings: &Settings, tiebreak: &dyn Tiebreaker) -> Reading {
    let mut reading = Reading::default();
    if !tiebreak.available() {
        reading.questions.push(Question::PasteAsTable);
        return reading;
    }
    // "Week of 10/12" anywhere anchors the weekdays; otherwise this week, or
    // the coming one on a weekend.
    let anchor = text
        .lines()
        .find_map(|l| dates::week_anchor(l, settings.today))
        .unwrap_or_else(|| {
            use chrono::Datelike;
            let t = settings.today;
            let monday = t - Duration::days(t.weekday().num_days_from_monday() as i64);
            if matches!(t.weekday(), Weekday::Sat | Weekday::Sun) {
                monday + Duration::days(7)
            } else {
                monday
            }
        });
    let mut people: Vec<String> = Vec::new();
    let mut sites: Vec<String> = Vec::new();
    let mut unread = 0usize;
    let lines = text
        .lines()
        .enumerate()
        .flat_map(|(row, raw)| pieces(&clean(raw)).into_iter().map(move |p| (row, p)));
    for (row, line) in lines {
        if dates::week_anchor(&line, settings.today).is_some() {
            continue;
        }
        let Some(entries) = tiebreak.free_text(&line, &people, &sites) else {
            unread += 1;
            continue;
        };
        let line_key = key(&line);
        if !entries
            .iter()
            .all(|e| in_line(&line_key, &e.person) && in_line(&line_key, &e.site))
        {
            unread += 1;
            continue;
        }
        for e in entries {
            let Some(w) = weekday(&e.day) else { continue };
            let date = dates::weekdays_in_week(&[w], anchor)[0];
            let mut a = Assignment::new(
                date,
                CellRef {
                    sheet: 0,
                    row: row as u32,
                    col: 0,
                },
            );
            a.person = Some(clean(&e.person));
            a.site = Some(clean(&e.site));
            if let Some(range) = e.time.as_deref().and_then(times::parse_range) {
                a.start = Some(range.start);
                a.end = Some(range.end);
            }
            if !people.contains(&e.person) {
                people.push(e.person.clone());
            }
            if !sites.contains(&e.site) {
                sites.push(e.site.clone());
            }
            reading.assignments.push(a);
        }
    }
    if reading.assignments.is_empty() {
        reading.questions.push(Question::PasteAsTable);
        return reading;
    }
    if unread > 0 {
        reading.notes.push(format!(
            "{unread} pasted line{} couldn't be read as a schedule and {} skipped.",
            if unread == 1 { "" } else { "s" },
            if unread == 1 { "was" } else { "were" }
        ));
    }
    let dates: BTreeSet<NaiveDate> = reading.assignments.iter().map(|a| a.date).collect();
    if let (Some(first), Some(last)) = (dates.first(), dates.last()) {
        let mut d = *first;
        while d <= *last {
            reading.window.push(d);
            d += Duration::days(1);
        }
    }
    reading.sheet = "Pasted".into();
    reading
}

#[cfg(test)]
mod tests {
    use super::pieces;

    #[test]
    fn splits_run_together_days_only() {
        assert_eq!(
            pieces("Mon: Mike & Jose @ Johnson. Tue: Mike - Smith job"),
            ["Mon: Mike & Jose @ Johnson", "Tue: Mike - Smith job"]
        );
        assert_eq!(
            pieces("Mike @ 1420 Johnson St. Unit B"),
            ["Mike @ 1420 Johnson St. Unit B"]
        );
        assert_eq!(pieces("Wed: Ana; Thu: Luis."), ["Wed: Ana", "Thu: Luis."]);
    }
}
