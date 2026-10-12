//! The week grid the app draws: people as rows, days as columns, what each
//! person is doing in each cell, colored by the flags; then the job rows,
//! with the days nobody is on a job marked. Everything here is seen through
//! the merges, so "Mike B" and "Michael Baker" share one row.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::classify::{EntityKind, entity_kind};
use crate::identity::is_internal;
use crate::matching::Matching;
use crate::model::{Assignment, Flag, FlagKind, Part, Reading, Status};
use crate::settings::Settings;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WeekView {
    pub dates: Vec<NaiveDate>,
    /// Per date: a working day under the current settings.
    pub working: Vec<bool>,
    pub people: Vec<PersonRow>,
    pub jobs: Vec<JobRow>,
    /// Per assignment in `Reading::assignments`: the matched person and job
    /// names, for the "Cleaned schedule" sheet. Not sent to the page.
    #[serde(skip)]
    pub cleaned: Vec<(Option<String>, Option<String>)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersonRow {
    pub name: String,
    pub kind: EntityKind,
    pub cells: Vec<PersonCell>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PersonCell {
    /// What the schedule says for that day: sites (with times) and statuses.
    pub items: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark: Option<Mark>,
}

/// Red: double-booked or assigned while off. Amber: not booked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mark {
    Red,
    Amber,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobRow {
    pub name: String,
    pub cells: Vec<JobCell>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JobCell {
    pub people: Vec<String>,
    /// Active that day with nobody on it (a flag).
    pub unstaffed: bool,
}

fn short_time(m: u16) -> String {
    let (h, min) = (m / 60, m % 60);
    let (h12, ap) = match h {
        0 => (12, "a"),
        1..=11 => (h, "a"),
        12 => (12, "p"),
        _ => (h - 12, "p"),
    };
    if min == 0 {
        format!("{h12}{ap}")
    } else {
        format!("{h12}:{min:02}{ap}")
    }
}

pub fn status_label(s: Status) -> &'static str {
    match s {
        Status::Off => "Off",
        Status::Pto => "PTO",
        Status::Vacation => "Vacation",
        Status::Sick => "Sick",
        Status::Holiday => "Holiday",
        Status::Out => "Out",
    }
}

fn part_label(p: Part) -> &'static str {
    match p {
        Part::Am => "AM",
        Part::Pm => "PM",
        Part::Half => "half day",
    }
}

/// One assignment as a cell line: "Smith remodel 7a–3p", "PTO (AM)".
pub fn item_text(a: &Assignment, site: Option<&str>) -> String {
    let mut s = match (a.status, site) {
        (Some(st), _) => status_label(st).to_string(),
        (None, Some(site)) => site.to_string(),
        (None, None) => String::new(),
    };
    if let (Some(start), Some(end)) = (a.start, a.end) {
        s.push_str(&format!(" {}–{}", short_time(start), short_time(end)));
    } else if let Some(p) = a.part {
        s.push_str(&format!(" ({})", part_label(p)));
    }
    s
}

pub fn build(
    reading: &Reading,
    settings: &Settings,
    matching: &Matching,
    flags: &[Flag],
) -> WeekView {
    let dates = reading.window.clone();
    let col: BTreeMap<NaiveDate, usize> = dates.iter().enumerate().map(|(i, d)| (*d, i)).collect();
    let working = dates.iter().map(|d| settings.is_working_date(*d)).collect();

    // People, in the order the file first names them.
    let mut order: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut add = |pk: String, order: &mut Vec<String>| {
        if seen.insert(pk.clone()) {
            order.push(pk);
        }
    };
    if let Some(roster) = &reading.roster {
        for n in roster {
            add(matching.person(n), &mut order);
        }
    }
    let mut cells: BTreeMap<String, Vec<PersonCell>> = BTreeMap::new();
    let mut job_order: Vec<String> = Vec::new();
    let mut job_cells: BTreeMap<String, Vec<JobCell>> = BTreeMap::new();
    let mut jobs_seen = BTreeSet::new();
    for a in &reading.assignments {
        let Some(&c) = col.get(&a.date) else { continue };
        let site = a.site.as_deref().map(|s| matching.site(s));
        let site_name = site.as_deref().map(|s| matching.sites.display(s));
        if let Some(sk) = &site
            && !is_internal(site_name.as_deref().unwrap_or(""))
            && jobs_seen.insert(sk.clone())
        {
            job_order.push(sk.clone());
        }
        if let Some(p) = a.person.as_deref() {
            let pk = matching.person(p);
            add(pk.clone(), &mut order);
            let row = cells
                .entry(pk.clone())
                .or_insert_with(|| vec![PersonCell::default(); dates.len()]);
            let text = item_text(a, site_name.as_deref());
            if !text.is_empty() && !row[c].items.contains(&text) {
                row[c].items.push(text);
            }
            if let (Some(sk), None) = (&site, a.status) {
                let jrow = job_cells
                    .entry(sk.clone())
                    .or_insert_with(|| vec![JobCell::default(); dates.len()]);
                let who = matching.people.display(&pk);
                if !jrow[c].people.contains(&who) {
                    jrow[c].people.push(who);
                }
            }
        }
    }
    for job in &reading.grid_jobs {
        let sk = matching.site(job);
        if jobs_seen.insert(sk.clone()) {
            job_order.push(sk);
        }
    }

    let mut people: Vec<PersonRow> = order
        .iter()
        .map(|pk| {
            let name = matching.people.display(pk);
            PersonRow {
                kind: entity_kind(&name),
                cells: cells
                    .remove(pk)
                    .unwrap_or_else(|| vec![PersonCell::default(); dates.len()]),
                name,
            }
        })
        .collect();
    let mut jobs: Vec<JobRow> = job_order
        .iter()
        .map(|sk| JobRow {
            name: matching.sites.display(sk),
            cells: job_cells
                .remove(sk)
                .unwrap_or_else(|| vec![JobCell::default(); dates.len()]),
        })
        .collect();

    for f in flags {
        let Some(&c) = col.get(&f.date) else { continue };
        match f.kind {
            FlagKind::DoubleBooked | FlagKind::AssignedWhileOff | FlagKind::NotBooked => {
                let mark = if f.kind == FlagKind::NotBooked {
                    Mark::Amber
                } else {
                    Mark::Red
                };
                if let Some(row) = people
                    .iter_mut()
                    .find(|r| Some(&r.name) == f.person.as_ref())
                    && row.cells[c].mark != Some(Mark::Red)
                {
                    row.cells[c].mark = Some(mark);
                }
            }
            FlagKind::Unstaffed => {
                if let Some(row) = jobs.iter_mut().find(|r| Some(&r.name) == f.site.as_ref()) {
                    row.cells[c].unstaffed = true;
                }
            }
        }
    }
    // Placeholders and subcontractors after the crew.
    people.sort_by_key(|r| r.kind != EntityKind::Employee);
    let cleaned = reading
        .assignments
        .iter()
        .map(|a| {
            (
                a.person
                    .as_deref()
                    .map(|p| matching.people.display(&matching.person(p))),
                a.site
                    .as_deref()
                    .map(|s| matching.sites.display(&matching.site(s))),
            )
        })
        .collect();
    WeekView {
        dates,
        working,
        people,
        jobs,
        cleaned,
    }
}
