//! Stages 8–9: who is who (employee, placeholder, subcontractor), then the
//! four flags and the information items, exactly as fixtures/FORMAT.md
//! "Flag semantics" defines them.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use chrono::{Datelike, Duration, NaiveDate};

use crate::classify::{EntityKind, entity_kind};
use crate::identity::is_internal;
use crate::matching::{Matching, MergeKind};
use crate::model::{Assignment, Flag, FlagKind, Info, InfoKind, Part, Reading, Status};
use crate::settings::Settings;

/// One work record seen through identities.
#[derive(Clone, Debug)]
struct Work {
    site: String,
    start: Option<u16>,
    end: Option<u16>,
    part: Option<Part>,
}

impl Work {
    fn of(a: &Assignment, matching: &Matching) -> Work {
        Work {
            site: matching.site(a.site.as_deref().unwrap_or("")),
            start: a.start,
            end: a.end,
            part: a.part,
        }
    }

    /// Minutes [from, to) after midnight; past midnight runs into tomorrow.
    fn interval(&self) -> Option<(u32, u32)> {
        match (self.start, self.end, self.part) {
            (Some(s), Some(e), _) => {
                let (s, e) = (s as u32, e as u32);
                Some((s, if e <= s { e + 1440 } else { e }))
            }
            (_, _, Some(Part::Am)) => Some((0, 720)),
            (_, _, Some(Part::Pm)) => Some((720, 1440)),
            _ => None,
        }
    }

    fn untimed(&self) -> bool {
        self.start.is_none() && self.part.is_none()
    }
}

/// Two entries clash when either has no time and no part, or their times
/// overlap. A half day without hours clashes with nothing that has a part
/// or time of its own.
fn clash(a: &Work, b: &Work) -> bool {
    if a.untimed() || b.untimed() {
        return true;
    }
    match (a.interval(), b.interval()) {
        (Some(x), Some(y)) => x.0 < y.1 && y.0 < x.1,
        _ => false,
    }
}

/// A status with a part leaves the other half of the day free.
fn status_clashes(part: Option<Part>, work: &Work) -> bool {
    let free = match part {
        None => return true,
        Some(Part::Half) => return false,
        Some(Part::Am) => (720, 2880),
        Some(Part::Pm) => (0, 720),
    };
    match work.interval() {
        Some((s, e)) => !(s >= free.0 && e <= free.1),
        None => true,
    }
}

/// A status and the part of the day it covers.
type StatusMark = (Status, Option<Part>);

fn day(d: NaiveDate) -> String {
    d.format("%a %-m/%-d").to_string()
}

fn list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        _ => format!(
            "{} and {}",
            names[..names.len() - 1].join(", "),
            names[names.len() - 1]
        ),
    }
}

fn status_word(s: Status) -> &'static str {
    match s {
        Status::Off => "off",
        Status::Pto => "on PTO",
        Status::Vacation => "on vacation",
        Status::Sick => "out sick",
        Status::Holiday => "off for the holiday",
        Status::Out => "out",
    }
}

fn week_of(d: NaiveDate) -> NaiveDate {
    d - Duration::days(d.weekday().num_days_from_monday() as i64)
}

pub fn analyze(
    reading: &Reading,
    settings: &Settings,
    matching: &Matching,
) -> (Vec<Flag>, Vec<Info>) {
    let people = &matching.people;
    let sites = &matching.sites;
    let mut work: BTreeMap<(String, NaiveDate), Vec<Work>> = BTreeMap::new();
    let mut statuses: BTreeMap<(String, NaiveDate), Vec<StatusMark>> = BTreeMap::new();
    let mut staffed: HashSet<(String, NaiveDate)> = HashSet::new();
    let mut explicit: BTreeSet<(String, NaiveDate)> = BTreeSet::new();
    let mut job_dates: BTreeMap<String, BTreeSet<NaiveDate>> = BTreeMap::new();

    for a in &reading.assignments {
        let pk = a.person.as_deref().map(|p| matching.person(p));
        let sk = a.site.as_deref().map(|s| matching.site(s));
        match (pk, sk, a.status, a.unstaffed) {
            (_, Some(sk), _, true) => {
                explicit.insert((sk.clone(), a.date));
                job_dates.entry(sk).or_default().insert(a.date);
            }
            (Some(pk), _, Some(status), _) => statuses
                .entry((pk, a.date))
                .or_default()
                .push((status, a.part)),
            (Some(pk), Some(sk), None, false) => {
                staffed.insert((sk.clone(), a.date));
                job_dates.entry(sk).or_default().insert(a.date);
                work.entry((pk, a.date))
                    .or_default()
                    .push(Work::of(a, matching));
            }
            _ => {}
        }
    }

    // The roster block if there is one, otherwise everyone seen.
    let roster: BTreeSet<String> = match &reading.roster {
        Some(names) => names.iter().map(|n| matching.person(n)).collect(),
        None => work
            .keys()
            .chain(statuses.keys())
            .map(|(pk, _)| pk.clone())
            .collect(),
    };
    let kind = |pk: &str| entity_kind(&people.display(pk));
    let working: Vec<NaiveDate> = reading
        .window
        .iter()
        .copied()
        .filter(|d| settings.is_working_date(*d))
        .collect();
    let mut flags = Vec::new();
    let mut info = Vec::new();

    // Double-booked, split days, shop + site, duplicates, subcontractors.
    for ((pk, date), entries) in &work {
        let who = people.display(pk);
        let k = kind(pk);
        if k == EntityKind::Placeholder {
            continue;
        }
        let mut distinct: Vec<&str> = Vec::new();
        for e in entries {
            if !distinct.contains(&e.site.as_str()) {
                distinct.push(&e.site);
            }
        }
        let jobs: Vec<&str> = distinct
            .iter()
            .copied()
            .filter(|s| !is_internal(&sites.display(s)))
            .collect();
        let internal: Vec<&str> = distinct
            .iter()
            .copied()
            .filter(|s| is_internal(&sites.display(s)))
            .collect();
        let shown = |keys: &[&str]| keys.iter().map(|s| sites.display(s)).collect::<Vec<_>>();

        if k == EntityKind::Subcontractor {
            if jobs.len() >= 2 {
                let names = shown(&jobs);
                info.push(Info {
                    kind: InfoKind::SubcontractorMultiSite,
                    date: *date,
                    person: who.clone(),
                    message: format!(
                        "{who} is at {} on {}. That's normal for a subcontractor.",
                        list(&names),
                        day(*date)
                    ),
                    sites: names,
                });
            }
            continue;
        }

        for site in &distinct {
            let here: Vec<&Work> = entries.iter().filter(|e| e.site == *site).collect();
            let dup = here
                .iter()
                .enumerate()
                .any(|(i, a)| here[i + 1..].iter().any(|b| clash(a, b)));
            if dup {
                let name = sites.display(site);
                info.push(Info {
                    kind: InfoKind::DuplicateEntry,
                    date: *date,
                    person: who.clone(),
                    message: format!("{who} is listed twice at {name} on {}.", day(*date)),
                    sites: vec![name],
                });
            }
        }

        if jobs.len() >= 2 {
            let mut clashing: Vec<&str> = Vec::new();
            for (i, a) in entries.iter().enumerate() {
                for b in &entries[i + 1..] {
                    if a.site != b.site
                        && jobs.contains(&a.site.as_str())
                        && jobs.contains(&b.site.as_str())
                        && clash(a, b)
                    {
                        for s in [a.site.as_str(), b.site.as_str()] {
                            if !clashing.contains(&s) {
                                clashing.push(s);
                            }
                        }
                    }
                }
            }
            if !clashing.is_empty() {
                let order: Vec<&str> = jobs
                    .iter()
                    .copied()
                    .filter(|s| clashing.contains(s))
                    .collect();
                let names = shown(&order);
                flags.push(Flag {
                    kind: FlagKind::DoubleBooked,
                    date: *date,
                    person: Some(who.clone()),
                    site: None,
                    message: format!("{who} is at {} on {}.", list(&names), day(*date)),
                    sites: names,
                    status: None,
                    explicit: false,
                    merges: Vec::new(),
                });
            } else {
                let names = shown(&distinct);
                info.push(Info {
                    kind: InfoKind::SplitDay,
                    date: *date,
                    person: who.clone(),
                    message: format!(
                        "{who} splits {} between {} at different times.",
                        day(*date),
                        list(&names)
                    ),
                    sites: names,
                });
            }
        } else if (jobs.len() == 1 && !internal.is_empty())
            || (jobs.is_empty() && internal.len() >= 2)
        {
            let names = shown(&distinct);
            info.push(Info {
                kind: InfoKind::InternalPlusSite,
                date: *date,
                person: who.clone(),
                message: format!("{who} is at {} on {}.", list(&names), day(*date)),
                sites: names,
            });
        }
    }

    // Assigned while off.
    for ((pk, date), marks) in &statuses {
        if kind(pk) != EntityKind::Employee {
            continue;
        }
        let Some(entries) = work.get(&(pk.clone(), *date)) else {
            continue;
        };
        for (status, part) in marks {
            let clashing: Vec<&Work> = entries
                .iter()
                .filter(|w| status_clashes(*part, w))
                .collect();
            if clashing.is_empty() {
                continue;
            }
            let who = people.display(pk);
            let mut names: Vec<String> = Vec::new();
            for w in clashing {
                let n = sites.display(&w.site);
                if !names.contains(&n) {
                    names.push(n);
                }
            }
            flags.push(Flag {
                kind: FlagKind::AssignedWhileOff,
                date: *date,
                person: Some(who.clone()),
                site: None,
                message: format!(
                    "{who} is {} on {} but is also on {}.",
                    status_word(*status),
                    day(*date),
                    list(&names)
                ),
                sites: names,
                status: Some(*status),
                explicit: false,
                merges: Vec::new(),
            });
            break;
        }
    }

    // Not booked: roster employees with nothing on a working day.
    for pk in &roster {
        if kind(pk) != EntityKind::Employee {
            continue;
        }
        for d in &working {
            let key = (pk.clone(), *d);
            if !work.contains_key(&key) && !statuses.contains_key(&key) {
                let who = people.display(pk);
                flags.push(Flag {
                    kind: FlagKind::NotBooked,
                    date: *d,
                    person: Some(who.clone()),
                    site: None,
                    sites: Vec::new(),
                    status: None,
                    explicit: false,
                    merges: Vec::new(),
                    message: format!("{who} has nothing on {}.", day(*d)),
                });
            }
        }
    }

    // Unstaffed: explicit markers, then job-days with nobody on them.
    let mut unstaffed: BTreeSet<(NaiveDate, String)> = BTreeSet::new();
    for (sk, d) in &explicit {
        unstaffed.insert((*d, sk.clone()));
    }
    let inactive: HashSet<(String, NaiveDate)> = reading
        .inactive
        .iter()
        .map(|(s, d)| (matching.site(s), *d))
        .collect();
    let mut inferred: BTreeSet<(NaiveDate, String)> = BTreeSet::new();
    if !reading.grid_jobs.is_empty() {
        for job in &reading.grid_jobs {
            let sk = matching.site(job);
            if is_internal(job) {
                continue;
            }
            for d in &working {
                let key = (sk.clone(), *d);
                if !inactive.contains(&key) && !staffed.contains(&key) && !explicit.contains(&key) {
                    inferred.insert((*d, sk.clone()));
                }
            }
        }
    } else {
        for (sk, dates) in &job_dates {
            if is_internal(&sites.display(sk)) {
                continue;
            }
            let mut by_week: BTreeMap<NaiveDate, Vec<NaiveDate>> = BTreeMap::new();
            for d in dates.iter().filter(|d| settings.is_working_date(**d)) {
                by_week.entry(week_of(*d)).or_default().push(*d);
            }
            for days in by_week.values() {
                let (first, last) = (days[0], days[days.len() - 1]);
                for d in working.iter().filter(|d| **d >= first && **d <= last) {
                    let key = (sk.clone(), *d);
                    if !staffed.contains(&key) && !explicit.contains(&key) {
                        inferred.insert((*d, sk.clone()));
                    }
                }
            }
        }
    }
    let marked = unstaffed.into_iter().map(|x| (x, true));
    for ((date, sk), was_marked) in marked.chain(inferred.into_iter().map(|x| (x, false))) {
        let name = sites.display(&sk);
        flags.push(Flag {
            kind: FlagKind::Unstaffed,
            date,
            person: None,
            message: if was_marked {
                format!("{name} still needs someone on {}.", day(date))
            } else {
                format!("Nobody is on {name} on {}.", day(date))
            },
            site: Some(name),
            sites: Vec::new(),
            status: None,
            explicit: was_marked,
            merges: Vec::new(),
        });
    }

    // Chips: the merges behind each finding's person and sites.
    for f in &mut flags {
        f.merges = matching
            .merges
            .iter()
            .enumerate()
            .filter(|(_, m)| match m.kind {
                MergeKind::Person => f.person.as_deref() == Some(m.into.as_str()),
                MergeKind::Site => {
                    f.sites.contains(&m.into) || f.site.as_deref() == Some(m.into.as_str())
                }
            })
            .map(|(i, _)| i)
            .collect();
    }
    flags.sort_by(|a, b| {
        (a.date, a.kind, &a.person, &a.site).cmp(&(b.date, b.kind, &b.person, &b.site))
    });
    info.sort_by(|a, b| (a.date, a.kind, &a.person).cmp(&(b.date, b.kind, &b.person)));
    (flags, info)
}
