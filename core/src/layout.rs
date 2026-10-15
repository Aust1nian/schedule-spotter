//! Stages 3–6: find the schedule in a sheet, decide its layout and column
//! roles, and turn its cells into assignments.
//!
//! Every reader (day-axis grids in both orientations, long lists, calendar
//! months) proposes a reading with a quality score. The best one wins; if the
//! top two are too close to call, the owner is asked, with the best reading
//! applied meanwhile.

use std::collections::{BTreeSet, HashMap, HashSet};

use chrono::{Datelike, Duration, NaiveDate, Weekday};

use crate::cells::{self, Expect, Item, Parsed};
use crate::classify::person_score;
use crate::dates;
use crate::grid::Grid;
use crate::lexicon::{crew_name, is_crew_roster_title, is_roster_title, status_word};
use crate::model::{
    Answer, Answers, Arrangement, Assignment, CellRef, ColumnReading, Layout, Part, Question,
    Reading, Status,
};
use crate::read::Workbook;
use crate::settings::Settings;
use crate::text::{clean, key};
use crate::times::TimeRange;

/// Crew name (key) → its display name and members as written.
#[derive(Clone, Debug, Default)]
pub struct Crews {
    pub map: HashMap<String, (String, Vec<String>)>,
}

impl Crews {
    pub fn members(&self, name: &str) -> Option<&Vec<String>> {
        self.map.get(&key(name)).map(|(_, m)| m)
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// What every reader shares about one sheet orientation.
pub struct SheetCtx<'a> {
    pub g: &'a Grid,
    pub today: NaiveDate,
    /// "Week of …" anchors: (row, col, date).
    pub anchors: Vec<(usize, usize, NaiveDate)>,
    pub crews: Crews,
    /// Cells that belong to crew or roster blocks, not the schedule.
    pub consumed: HashSet<(usize, usize)>,
    pub roster: Option<Vec<String>>,
    /// Some numeric date in the sheet has a first part over 12 (and none a
    /// second part over 12): the author writes day-first.
    pub day_first: bool,
    /// The owner's answers so far.
    pub answers: &'a Answers,
    /// The model, for cells and columns the rules can't settle (T3, T4).
    pub tiebreak: &'a dyn crate::matching::Tiebreaker,
}

impl<'a> SheetCtx<'a> {
    pub fn new(
        g: &'a Grid,
        today: NaiveDate,
        answers: &'a Answers,
        tiebreak: &'a dyn crate::matching::Tiebreaker,
    ) -> SheetCtx<'a> {
        let mut anchors = Vec::new();
        for r in 0..g.height() {
            for c in 0..g.width() {
                let t = g.text(r, c);
                if t.len() >= 6
                    && t.len() <= 80
                    && let Some(d) = dates::week_anchor(t, today)
                {
                    anchors.push((r, c, d));
                }
            }
        }
        let mut consumed = HashSet::new();
        let crews = find_crews(g, &mut consumed);
        let roster = find_roster(g, &mut consumed);
        let numeric: Vec<dates::DayTok> = (0..g.height())
            .flat_map(|r| (0..g.width()).filter_map(move |c| g.day(r, c)))
            .filter(|t| matches!(t, dates::DayTok::Numeric { .. }))
            .collect();
        let day_first = dates::day_first(&numeric);
        SheetCtx {
            g,
            today,
            anchors,
            crews,
            consumed,
            roster,
            day_first,
            answers,
            tiebreak,
        }
    }

    /// The "Week of" anchor that governs `row`: the nearest one above it,
    /// preferring one over the same columns.
    pub fn anchor_for(&self, row: usize, cols: std::ops::Range<usize>) -> Option<NaiveDate> {
        let above: Vec<_> = self.anchors.iter().filter(|(r, _, _)| *r <= row).collect();
        let near_cols = above
            .iter()
            .filter(|(_, c, _)| *c + 2 >= cols.start && *c < cols.end)
            .max_by_key(|(r, _, _)| *r);
        near_cols
            .or_else(|| {
                above
                    .iter()
                    .filter(|(_, c, _)| *c < cols.end)
                    .max_by_key(|(r, c, _)| (*r, *c))
            })
            .or_else(|| above.iter().max_by_key(|(r, _, _)| *r))
            .map(|(_, _, d)| *d)
            .or_else(|| self.anchors.first().map(|(_, _, d)| *d))
    }

    /// With no anchor anywhere, weekday names mean the coming week.
    pub fn default_anchor(&self) -> NaiveDate {
        let t = self.today;
        let back = t.weekday().num_days_from_monday() as i64;
        let monday = t - Duration::days(back);
        if matches!(t.weekday(), Weekday::Sat | Weekday::Sun) {
            monday + Duration::days(7)
        } else {
            monday
        }
    }
}

/// Collects one reading's output.
#[derive(Default)]
pub struct Sink {
    pub assignments: Vec<Assignment>,
    pub window: BTreeSet<NaiveDate>,
    pub grid_jobs: Vec<String>,
    pub inactive: Vec<(String, NaiveDate)>,
    pub questions: Vec<Question>,
    pub notes: Vec<String>,
    pub columns: Vec<ColumnReading>,
    /// Non-empty data cells that produced something, and that didn't.
    pub understood: usize,
    pub junk: usize,
    pub used_crews: bool,
    /// Names from an employee roster block on the sheet.
    pub roster: Option<Vec<String>>,
    /// Every way the sheet could be read, best first (for "What I read").
    pub layout_options: Vec<String>,
}

/// Per-record extras gathered from the cell and its column.
#[derive(Clone, Debug, Default)]
pub struct Extras {
    pub time: Option<TimeRange>,
    pub part: Option<Part>,
    pub note: Option<String>,
}

impl Sink {
    fn record(&mut self, date: NaiveDate, src: CellRef) -> Assignment {
        Assignment::new(date, src)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn work(
        &mut self,
        crews: &Crews,
        person: &str,
        site: &str,
        date: NaiveDate,
        item: &Item,
        extras: &Extras,
        src: CellRef,
    ) {
        let names: Vec<String> = match crews.members(person) {
            Some(members) => {
                self.used_crews = true;
                members.clone()
            }
            None => vec![clean(person)],
        };
        // A date, a day or a bare number is never a person or a site; a
        // short day abbreviation that's also a first name ("Mo") is a name.
        let not_a_name =
            |t: &str| crate::dates::is_day_not_name(t) || t.trim().parse::<f64>().is_ok();
        if not_a_name(person) || not_a_name(site) {
            self.junk += 1;
            return;
        }
        for name in names {
            let mut a = self.record(date, src);
            a.person = Some(name);
            a.site = Some(clean(site));
            let time = item.time.or(extras.time);
            a.start = time.map(|t| t.start);
            a.end = time.map(|t| t.end);
            a.part = item.part.or(extras.part);
            a.note = join_notes(item.note.as_deref(), extras.note.as_deref());
            self.assignments.push(a);
        }
    }

    pub fn status(
        &mut self,
        crews: &Crews,
        person: &str,
        date: NaiveDate,
        (status, part): (Status, Option<Part>),
        note: Option<&str>,
        src: CellRef,
    ) {
        let names: Vec<String> = match crews.members(person) {
            Some(members) => members.clone(),
            None => vec![clean(person)],
        };
        for name in names {
            let mut a = self.record(date, src);
            a.person = Some(name);
            a.status = Some(status);
            a.part = part;
            a.note = note.map(str::to_string);
            self.assignments.push(a);
        }
    }

    pub fn unstaffed(&mut self, site: &str, date: NaiveDate, src: CellRef) {
        let mut a = self.record(date, src);
        a.site = Some(clean(site));
        a.unstaffed = true;
        self.assignments.push(a);
    }

    pub fn grid_job(&mut self, site: &str) {
        let site = clean(site);
        if !self.grid_jobs.iter().any(|j| key(j) == key(&site)) {
            self.grid_jobs.push(site);
        }
    }

    pub fn ask(&mut self, q: Question) {
        if !self.questions.contains(&q) {
            self.questions.push(q);
        }
    }

    pub fn quality(&self) -> f32 {
        let total = self.understood + self.junk;
        if total == 0 {
            0.0
        } else {
            self.understood as f32 / total as f32
        }
    }
}

fn join_notes(a: Option<&str>, b: Option<&str>) -> Option<String> {
    match (a, b) {
        (Some(a), Some(b)) if a != b => Some(format!("{a}; {b}")),
        (Some(a), _) => Some(a.to_string()),
        (None, Some(b)) => Some(b.to_string()),
        (None, None) => None,
    }
}

/// A proposed reading of one sheet.
pub struct Candidate {
    pub layout: Layout,
    pub arrangement: Arrangement,
    pub sink: Sink,
}

impl Candidate {
    /// How much the people in this reading look like people: a sheet of
    /// suppliers or materials with dates can parse as a list, but its
    /// "names" give it away.
    pub fn person_likeness(&self) -> f32 {
        let mut names: Vec<&str> = self
            .sink
            .assignments
            .iter()
            .filter_map(|a| a.person.as_deref())
            .collect();
        names.sort_unstable();
        names.dedup();
        if names.is_empty() {
            return 0.0;
        }
        names.iter().map(|n| person_score(n)).sum::<f32>() / names.len() as f32
    }

    pub fn score(&self) -> f32 {
        let n = self.sink.assignments.len() as f32;
        if n == 0.0 {
            return 0.0;
        }
        let people_count = self.sink.assignments.iter().any(|a| a.person.is_some());
        let likeness = if people_count {
            self.person_likeness()
        } else {
            1.0
        };
        self.sink.quality() * (n / (n + 2.0)) * (0.5 + 0.5 * likeness)
    }
}

/// A sheet whose header has two or more order-list words ("Item", "Qty",
/// "Vendor", "Price"): a materials or purchase list, not a schedule.
fn looks_like_order_sheet(g: &Grid) -> bool {
    (0..g.height().min(6)).any(|r| {
        let words: HashSet<String> = (0..g.width())
            .map(|c| key(g.text(r, c)))
            .map(|k| k.trim_end_matches([':', '.', '#', ' ']).trim().to_string())
            .filter(|k| {
                matches!(
                    k.as_str(),
                    "item"
                        | "items"
                        | "qty"
                        | "quantity"
                        | "supplier"
                        | "vendor"
                        | "price"
                        | "unit price"
                        | "cost"
                        | "sku"
                        | "part"
                        | "part no"
                        | "material"
                        | "materials"
                        | "description"
                        | "amount"
                        | "po"
                        | "order no"
                        | "cantidad"
                        | "proveedor"
                        | "precio"
                )
            })
            .collect();
        words.len() >= 2
    })
}

/// A sheet name that says it's the schedule.
fn schedule_sheet_name(name: &str) -> bool {
    let k = key(name);
    [
        "schedule",
        "horario",
        "calendar",
        "calendario",
        "crew",
        "week",
        "semana",
        "dispatch",
        "board",
    ]
    .iter()
    .any(|w| {
        k.split(|c: char| !c.is_alphanumeric())
            .any(|part| part == *w)
    })
}

/// Reads the workbook: picks the sheet and the reading, and asks when that
/// choice is a coin flip.
pub fn read_workbook(
    wb: &Workbook,
    settings: &Settings,
    answers: &Answers,
    tiebreak: &dyn crate::matching::Tiebreaker,
) -> Reading {
    let mut per_sheet: Vec<(usize, Candidate)> = Vec::new();
    let mut order_sheets: HashSet<usize> = HashSet::new();
    for (i, sheet) in wb.sheets.iter().enumerate() {
        let g = Grid::new(sheet, i as u16);
        if g.height() == 0 {
            continue;
        }
        if looks_like_order_sheet(&g) {
            order_sheets.insert(i);
        }
        if let Some(best) = read_sheet(&g, settings, answers, tiebreak) {
            per_sheet.push((i, best));
        }
    }
    let mut reading = Reading {
        notes: wb.notes.clone(),
        ..Reading::default()
    };
    if per_sheet.is_empty() {
        return reading;
    }
    // An order or materials list ("Item | Qty | For job | Vendor | Need by |
    // Ordered by") can read like a schedule: it has jobs, dates and first
    // names. It goes after the other sheets and never makes the choice a
    // coin flip.
    per_sheet.sort_by(|a, b| {
        order_sheets
            .contains(&a.0)
            .cmp(&order_sheets.contains(&b.0))
            .then(b.1.score().total_cmp(&a.1.score()))
            .then(a.0.cmp(&b.0))
    });
    let plausible: Vec<usize> = per_sheet
        .iter()
        .filter(|(i, c)| {
            !order_sheets.contains(i)
                && c.score() >= 0.5
                && c.sink.assignments.len() >= 3
                && c.person_likeness() >= 0.6
        })
        .map(|(i, _)| *i)
        .collect();
    // Of several plausible sheets, the one named like a schedule ("Schedule",
    // "Crew Schedule") is the schedule; two so named ("Week 10-12", "Week
    // 10-19") are still a question.
    let named: Vec<usize> = plausible
        .iter()
        .copied()
        .filter(|&i| schedule_sheet_name(&wb.sheets[i].name))
        .collect();
    let schedule_like = if named.len() == 1 { 1 } else { plausible.len() };
    if named.len() == 1
        && let Some(pos) = per_sheet.iter().position(|(i, _)| *i == named[0])
    {
        let item = per_sheet.remove(pos);
        per_sheet.insert(0, item);
    }
    // The owner picked a sheet.
    let picked = match answers.get_subject("pick_sheet") {
        Some(Answer::Pick(name)) => per_sheet
            .iter()
            .position(|(i, _)| crate::text::key(&wb.sheets[*i].name) == crate::text::key(name)),
        _ => None,
    };
    let (chosen_index, chosen) = per_sheet.remove(picked.unwrap_or(0));
    let chosen_name = wb.sheets[chosen_index].name.clone();
    if schedule_like >= 2 {
        let options = wb.sheets.iter().map(|s| s.name.clone()).collect();
        reading.questions.push(Question::PickSheet {
            options,
            chosen: chosen_name.clone(),
        });
    }
    reading.sheet = chosen_name;
    reading.layout = Some(chosen.layout);
    reading.arrangement = Some(chosen.arrangement);
    let mut sink = chosen.sink;
    reading.window = sink.window.iter().copied().collect();
    reading.assignments = std::mem::take(&mut sink.assignments);
    reading.grid_jobs = sink.grid_jobs;
    reading.roster = sink.roster;
    reading.inactive = sink.inactive;
    reading.columns = sink.columns;
    reading.layout_options = sink.layout_options;
    reading.questions.extend(sink.questions);
    reading.notes.extend(sink.notes);
    reading
}

/// A reader: (sheet context, transposed?) → candidate readings.
pub type Reader = fn(&SheetCtx, bool) -> Vec<Candidate>;

/// The readers, for the timing example: (name, reader).
#[doc(hidden)]
pub fn readers() -> Vec<(&'static str, Reader)> {
    vec![
        ("calendar", |c, _| crate::layout_calendar::read(c)),
        ("grid", crate::layout_grid::read),
        ("list", |c, _| crate::layout_list::read(c)),
    ]
}

/// The best reading of one sheet, or `None` if nothing in it reads as a
/// schedule.
pub fn read_sheet(
    g: &Grid,
    settings: &Settings,
    answers: &Answers,
    tiebreak: &dyn crate::matching::Tiebreaker,
) -> Option<Candidate> {
    let today = settings.today;
    let ctx = SheetCtx::new(g, today, answers, tiebreak);
    let transposed = g.transposed();
    let tctx = SheetCtx::new(&transposed, today, answers, tiebreak);

    let mut candidates: Vec<Candidate> = Vec::new();
    candidates.extend(crate::layout_calendar::read(&ctx));
    candidates.extend(crate::layout_grid::read(&ctx, false));
    candidates.extend(crate::layout_grid::read(&tctx, true));
    candidates.extend(crate::layout_list::read(&ctx));
    candidates.retain(|c| !c.sink.assignments.is_empty() || !c.sink.grid_jobs.is_empty());
    if candidates.is_empty() {
        return None;
    }
    candidates.sort_by(|a, b| b.score().total_cmp(&a.score()));
    let picked = match answers.get_subject("pick_layout") {
        Some(Answer::Pick(name)) => candidates
            .iter()
            .position(|c| layout_name(c.layout) == name.as_str()),
        _ => None,
    };
    let mut options: Vec<String> = Vec::new();
    for c in &candidates {
        let name = layout_name(c.layout).to_string();
        if !options.contains(&name) {
            options.push(name);
        }
    }
    let mut best = candidates.remove(picked.unwrap_or(0));
    best.sink.layout_options = options;
    if let Some(second) = candidates.first().filter(|_| picked.is_none()) {
        let close = best.score() - second.score() < 0.03 && second.score() > 0.5;
        if close && family(best.layout) != family(second.layout) {
            best.sink.ask(Question::PickLayout {
                options: vec![
                    layout_name(best.layout).into(),
                    layout_name(second.layout).into(),
                ],
                chosen: layout_name(best.layout).into(),
            });
        }
    }
    if let Some(roster) = &ctx.roster
        && !g.is_transposed()
    {
        best.sink.notes.push(format!(
            "Using the roster block ({} names) as the full crew list.",
            roster.len()
        ));
        best.sink.roster = Some(roster.clone());
    }
    if best.sink.used_crews {
        best.layout = Layout::CrewBased;
    }
    Some(best)
}

fn family(layout: Layout) -> u8 {
    match layout {
        Layout::LongList => 0,
        Layout::PersonByDay | Layout::DayByPerson => 1,
        Layout::JobByDay | Layout::DayByJob => 2,
        Layout::CrewBased => 3,
        Layout::CalendarMonth => 4,
    }
}

pub fn layout_name(layout: Layout) -> &'static str {
    match layout {
        Layout::LongList => "one row per assignment",
        Layout::PersonByDay => "people down the side, days across the top",
        Layout::DayByPerson => "days down the side, people across the top",
        Layout::JobByDay => "jobs down the side, days across the top",
        Layout::DayByJob => "days down the side, jobs across the top",
        Layout::CrewBased => "crews with a crew list",
        Layout::CalendarMonth => "a month calendar",
    }
}

// ------------------------------------------------------------ crews, roster

/// "Crew A: Mike, Jose", a crew name with members in the cells to its right,
/// or crew names heading columns of members.
fn find_crews(g: &Grid, consumed: &mut HashSet<(usize, usize)>) -> Crews {
    let mut crews = Crews::default();
    let add = |name: &str, members: Vec<String>, crews: &mut Crews| {
        let members: Vec<String> = members
            .into_iter()
            .map(|m| clean(&m))
            .filter(|m| !m.is_empty())
            .collect();
        if !members.is_empty() {
            crews
                .map
                .entry(key(name))
                .or_insert_with(|| (clean(name), members));
        }
    };
    for r in 0..g.height() {
        for c in 0..g.width() {
            let t = g.text(r, c);
            if t.is_empty() {
                continue;
            }
            // "Crew A: Mike, Jose, Luis"
            if let Some((left, right)) = t.split_once(':')
                && crew_name(&key(left))
                && !right.trim().is_empty()
            {
                let parsed = cells::parse_cell(right, Expect::People);
                let members: Vec<String> = parsed.items.into_iter().map(|i| i.text).collect();
                if members.iter().all(|m| person_score(m) >= 0.4) {
                    add(left, members, &mut crews);
                    consumed.insert((r, c));
                    continue;
                }
            }
            if !crew_name(&key(t)) {
                continue;
            }
            // Members to the right in the same row, and nothing day-like
            // there. Only the cells next to it in the file: a blank column
            // ends the list (the grid drops blank columns, so a roster block
            // beside the schedule would otherwise look like more members).
            let block = header_block(g, r, c);
            let right: Vec<usize> = block.into_iter().filter(|&cc| cc > c).collect();
            // The crew name leads its row ("Crew 1 | Mike | Jose"); a crew
            // name inside a schedule row is an assignment, not a list.
            let leads_row = g.first_nonempty(r) == Some(c);
            let row_is_roster = leads_row
                && !right.is_empty()
                && right.iter().all(|&cc| g.day(r, cc).is_none())
                && right.iter().all(|&cc| {
                    let p = cells::parse_cell(g.text(r, cc), Expect::People);
                    !p.items.is_empty()
                        && p.items
                            .iter()
                            .all(|i| person_score(&i.text) >= 0.4 && !crew_name(&key(&i.text)))
                        && p.status.is_none()
                })
                && (row_title_is_crew_list(g, r)
                    || right.len() >= 2
                    || g.text(r, c + 1).contains(','))
                // A crew lists each member once; the same "name" across the
                // row ("Crew A | Haskell Farm | Haskell Farm …") is a job.
                && {
                    let keys: Vec<String> = right.iter().map(|&cc| key(g.text(r, cc))).collect();
                    let distinct: HashSet<&String> = keys.iter().collect();
                    distinct.len() == keys.len()
                };
            if row_is_roster {
                let mut members = Vec::new();
                for &cc in &right {
                    members.extend(
                        cells::parse_cell(g.text(r, cc), Expect::People)
                            .items
                            .into_iter()
                            .map(|i| i.text),
                    );
                    consumed.insert((r, cc));
                }
                add(t, members, &mut crews);
                consumed.insert((r, c));
                continue;
            }
            // Columns headed by crew names ("Crew A | Crew B") with members
            // listed below. The block of headers holds nothing but crew
            // names (and an "Other" column). The block can share its row with
            // the schedule's own header when a blank column separates them
            // ("Crew | Mon … Fri | | Crew A | Crew B"); the grid drops blank
            // columns, so the gap shows in the file's own column numbers.
            let block = header_block(g, r, c);
            let peers = block
                .iter()
                .filter(|&&cc| cc != c && crew_name(&key(g.text(r, cc))))
                .count();
            let header_only_crews = block.iter().all(|&cc| {
                let k = key(g.text(r, cc));
                crew_name(&k)
                    || matches!(
                        k.as_str(),
                        "other" | "others" | "misc" | "extra" | "extras" | "floaters" | "helpers"
                    )
            });
            if peers >= 1 && header_only_crews {
                let mut members = Vec::new();
                let mut rr = r + 1;
                while rr < g.height()
                    && !g.is_empty(rr, c)
                    && person_score(g.text(rr, c)) >= 0.4
                    && !crew_name(&key(g.text(rr, c)))
                    && g.day(rr, c).is_none()
                {
                    members.push(g.text(rr, c).to_string());
                    consumed.insert((rr, c));
                    rr += 1;
                }
                if !members.is_empty() {
                    add(t, members, &mut crews);
                    consumed.insert((r, c));
                }
            }
        }
    }
    crews
}

/// The cells of row `r` around column `c` that sit next to each other in the
/// file, with no blank column between them.
fn header_block(g: &Grid, r: usize, c: usize) -> Vec<usize> {
    // Next to each other in the file, whichever way the grid is turned.
    let joined = |a: usize, b: usize| {
        let (sa, sb) = (g.source(r, a), g.source(r, b));
        !g.is_empty(r, a)
            && !g.is_empty(r, b)
            && ((sb.col == sa.col + 1 && sb.row == sa.row)
                || (sb.row == sa.row + 1 && sb.col == sa.col))
    };
    let mut lo = c;
    while lo > 0 && joined(lo - 1, lo) {
        lo -= 1;
    }
    let mut hi = c;
    while hi + 1 < g.width() && joined(hi, hi + 1) {
        hi += 1;
    }
    (lo..=hi).collect()
}

fn row_title_is_crew_list(g: &Grid, r: usize) -> bool {
    (0..r).rev().take(3).any(|rr| {
        g.row_nonempty(rr) == 1
            && g.first_nonempty(rr)
                .is_some_and(|c| is_crew_roster_title(&key(g.text(rr, c))))
    })
}

/// A title like "Roster" or "Employees" alone in its row, with names below.
fn find_roster(g: &Grid, consumed: &mut HashSet<(usize, usize)>) -> Option<Vec<String>> {
    for r in 0..g.height() {
        if g.row_nonempty(r) != 1 {
            continue;
        }
        let c = g.first_nonempty(r)?;
        if !is_roster_title(&key(g.text(r, c))) {
            continue;
        }
        let mut names = Vec::new();
        let mut rr = r + 1;
        while rr < g.height() {
            let t = g.text(rr, c);
            if t.is_empty()
                || is_roster_title(&key(t))
                || person_score(t) < 0.4
                || crew_name(&key(t))
                || status_word(&key(t)).is_some()
            {
                break;
            }
            // Comma lists in one cell, or one name per row.
            for item in cells::parse_cell(t, Expect::People).items {
                names.push(item.text);
            }
            for cc in 0..g.width() {
                consumed.insert((rr, cc));
            }
            rr += 1;
        }
        if !names.is_empty() {
            consumed.insert((r, c));
            return Some(names);
        }
    }
    // A "Roster" heading beside the schedule, names below it. When the
    // heading sits in a header row ("Name | Job | Staff"), the column may be
    // the list's own name column, so its cells stay readable and the roster
    // equals everyone seen. Anywhere else it's a separate block.
    for r in 0..g.height() {
        for c in 0..g.width() {
            if g.row_nonempty(r) < 2 {
                continue;
            }
            // A block of its own: a blank column between it and the cells to
            // its left ("Date | Tech | Job | | Technicians | Truck"). There a
            // people header ("Technicians") marks a roster too, never a
            // second name column for the list.
            let block = header_block(g, r, c);
            // A block with days in its header is another week's schedule
            // ("Name | Mon … | | Name | Mon …"), not a roster.
            // And the row is a list's header row: header words to the block's
            // left, at least one of them not a people header ("Date | Tech |
            // Job"). Not a data row whose next week starts with a "Laborer"
            // placeholder, nor side-by-side weeks that each repeat "Worker".
            let roles_left: Vec<crate::lexicon::HeaderRole> = (0..block[0])
                .filter_map(|cc| crate::lexicon::header_role(&key(g.text(r, cc))))
                .collect();
            let list_header = roles_left.len() >= 2
                && roles_left.iter().any(|h| {
                    !matches!(
                        h,
                        crate::lexicon::HeaderRole::Person | crate::lexicon::HeaderRole::Crew
                    )
                });
            let separate =
                block[0] > 0 && list_header && block.iter().all(|&cc| g.day(r, cc).is_none());
            let k = key(g.text(r, c));
            let people_header = matches!(
                crate::lexicon::header_role(&k),
                Some(crate::lexicon::HeaderRole::Person)
            );
            if !is_roster_title(&k) && !(separate && people_header) {
                continue;
            }
            let rows: Vec<usize> = (r + 1..g.height())
                .take_while(|&rr| {
                    let t = g.text(rr, c);
                    !t.is_empty()
                        && person_score(t) >= 0.4
                        && !crew_name(&key(t))
                        && status_word(&key(t)).is_none()
                })
                .collect();
            if rows.len() >= 2 {
                let header_row = (0..g.width())
                    .filter(|&cc| crate::lexicon::header_role(&key(g.text(r, cc))).is_some())
                    .count()
                    >= 2;
                if separate {
                    // The whole block goes ("Technicians | Truck").
                    for &cc in &block {
                        consumed.insert((r, cc));
                        for &rr in &rows {
                            consumed.insert((rr, cc));
                        }
                    }
                } else if !header_row {
                    consumed.insert((r, c));
                    for &rr in &rows {
                        consumed.insert((rr, c));
                    }
                }
                return Some(rows.iter().map(|&rr| clean(g.text(rr, c))).collect());
            }
        }
    }
    None
}

// ------------------------------------------------------------- shared helpers

/// Section titles inside a schedule: "Week of 9/28/2026", "OCTOBER 2026",
/// "Crew Schedule".
pub fn is_title_text(text: &str, today: NaiveDate) -> bool {
    let k = key(text);
    dates::week_anchor(text, today).is_some()
        || k.contains("schedule")
        || k.contains("horario")
        || k.starts_with("week ")
        || k.starts_with("semana ")
        || k.split(' ')
            .any(|w| crate::lexicon::is_month_word(w).is_some())
            && k.chars().any(|c| c.is_ascii_digit())
            && k.split(' ').count() <= 3
}

/// Rows that carry no schedule data: titles, footers, signature lines.
pub fn is_footer_text(text: &str) -> bool {
    let k = key(text);
    let words = k.split(' ').count();
    text.contains("___")
        || k.starts_with("approved")
        || k.starts_with("signed")
        || k.starts_with("signature")
        || k.starts_with("prepared by")
        || k.starts_with("notes:")
        || k.starts_with("note:")
        || k.starts_with("page ")
        || k.starts_with("printed")
        || k.starts_with("updated")
        || k.starts_with("last updated")
        || k.starts_with("*")
        || words > 8
        || text.len() > 70
}

/// Applies a parsed body cell for a person-grid row.
pub fn person_cell(
    sink: &mut Sink,
    crews: &Crews,
    person: &str,
    date: NaiveDate,
    parsed: &Parsed,
    extras: &Extras,
    src: CellRef,
) {
    let extras = Extras {
        time: parsed.time.or(extras.time),
        part: parsed.part.or(extras.part),
        note: join_notes(parsed.note.as_deref(), extras.note.as_deref()),
    };
    let mut any = false;
    for item in &parsed.items {
        sink.work(crews, person, &item.text, date, item, &extras, src);
        any = true;
    }
    if let Some((status, part)) = parsed.status {
        sink.status(
            crews,
            person,
            date,
            (status, part.or(if any { None } else { extras.part })),
            if any { None } else { extras.note.as_deref() },
            src,
        );
        any = true;
    }
    if any || parsed.unstaffed {
        sink.understood += 1;
    } else if !parsed.is_empty() || parsed.time.is_some() {
        sink.junk += 1;
    }
}

/// Applies a parsed body cell for a job-grid row.
pub fn job_cell(
    sink: &mut Sink,
    crews: &Crews,
    site: &str,
    date: NaiveDate,
    parsed: &Parsed,
    extras: &Extras,
    src: CellRef,
) {
    let extras = Extras {
        time: parsed.time.or(extras.time),
        part: parsed.part.or(extras.part),
        note: join_notes(parsed.note.as_deref(), extras.note.as_deref()),
    };
    let mut any = false;
    if let Some((status, part)) = parsed.status.filter(|_| !parsed.items.is_empty()) {
        // "Luis - off" inside a job's cell: Luis is off, not at the job.
        for item in &parsed.items {
            sink.status(
                crews,
                &item.text,
                date,
                (status, part),
                item.note.as_deref(),
                src,
            );
        }
        sink.understood += 1;
        return;
    }
    for item in &parsed.items {
        sink.work(crews, &item.text, site, date, item, &extras, src);
        any = true;
    }
    if parsed.unstaffed {
        sink.unstaffed(site, date, src);
        any = true;
    }
    if parsed.inactive && parsed.items.is_empty() {
        sink.inactive.push((clean(site), date));
        any = true;
    }
    if any {
        sink.understood += 1;
    } else if !parsed.is_empty() {
        sink.junk += 1;
    }
}

/// Merged-cell blanks in a label column: blanks directly under a label in
/// rows that still have data. Two or more such groups is a merged-cell
/// pattern and fills silently; a single group fills too but is asked about.
pub fn fill_down_labels(
    labels: &mut [(usize, String, bool)],
    header: &str,
    sink: &mut Sink,
    block_row: u32,
    answers: &Answers,
) {
    let question = Question::FillDown {
        column: header.to_string(),
        block_row,
    };
    if answers.get(&question) == Some(&Answer::No) {
        sink.ask(question);
        return;
    }
    // (row, label, row_has_data)
    let mut groups = 0;
    let mut in_group = false;
    for i in 1..labels.len() {
        let blank_with_data = labels[i].1.is_empty() && labels[i].2;
        if blank_with_data && (!labels[i - 1].1.is_empty() || in_group) {
            if !in_group {
                groups += 1;
            }
            in_group = true;
        } else {
            in_group = false;
        }
    }
    if groups == 0 {
        return;
    }
    let mut last = String::new();
    for entry in labels.iter_mut() {
        if entry.1.is_empty() && entry.2 && !last.is_empty() {
            entry.1 = last.clone();
        } else if !entry.1.is_empty() {
            last = entry.1.clone();
        }
    }
    if groups == 1 {
        sink.ask(question);
    }
}
