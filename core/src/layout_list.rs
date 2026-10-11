//! Long lists: one row per assignment (or per job with several people).
//!
//! Column roles come from content first and header words second, in
//! English or Spanish. A list may carry its dates in a date column, in a
//! weekday column under a "Week of" line, or in section rows ("MONDAY 10/12").

use std::collections::HashMap;

use chrono::{Datelike, Duration, NaiveDate};

use crate::cells::{self, Expect, Item};
use crate::classify::{person_score, site_score};
use crate::dates::{self, DayTok};
use crate::layout::{Candidate, Extras, SheetCtx, Sink, is_footer_text};
use crate::lexicon::{
    HeaderRole, crew_name, header_role, is_roster_title, is_total_label, is_unstaffed_marker,
    status_word,
};
use crate::model::{Arrangement, ColumnReading, Layout, Question};
use crate::read::Value;
use crate::text::{clean, key};
use crate::times;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Date,
    Day,
    Person,
    Site,
    Time,
    Start,
    End,
    Status,
    Note,
    Ignore,
}

impl Role {
    fn label(self) -> &'static str {
        match self {
            Role::Date => "Date",
            Role::Day => "Day",
            Role::Person => "Person",
            Role::Site => "Job",
            Role::Time => "Time",
            Role::Start => "Start time",
            Role::End => "End time",
            Role::Status => "Status",
            Role::Note => "Note",
            Role::Ignore => "Ignore",
        }
    }
}

#[derive(Default, Clone, Debug)]
struct Stats {
    n: usize,
    date: usize,
    weekday: usize,
    time: usize,
    single_time: usize,
    /// Clock times written as clock times ("7:30", "4pm"), not bare numbers.
    clock: usize,
    status: usize,
    numeric: usize,
    person: f32,
    site: f32,
    entity_n: usize,
    crew: usize,
}

const MAX_ROWS_BEFORE_CAP: usize = 20_000;

pub fn read(ctx: &SheetCtx) -> Vec<Candidate> {
    let g = ctx.g;
    if g.width() < 2 && !(g.width() == 1 && g.height() > 0) {
        return Vec::new();
    }
    let header = find_header(ctx);
    let start = header.map(|h| h + 1).unwrap_or(0);
    let data_rows: Vec<usize> = (start..g.height())
        .filter(|&r| is_data_row(ctx, r, header))
        .collect();
    let mut is_data = vec![false; g.height()];
    for &r in &data_rows {
        is_data[r] = true;
    }
    if data_rows.is_empty() {
        return Vec::new();
    }
    let stats: Vec<Stats> = (0..g.width())
        .map(|c| column_stats(ctx, c, &data_rows))
        .collect();
    let mut sink = Sink::default();
    let Some(roles) = assign_roles(ctx, header, &stats, &mut sink) else {
        return Vec::new();
    };

    let date_col = roles.iter().position(|r| *r == Role::Date);
    let day_col = roles.iter().position(|r| *r == Role::Day);
    let date_toks: Vec<DayTok> = date_col
        .map(|c| data_rows.iter().filter_map(|&r| g.day(r, c)).collect())
        .unwrap_or_default();
    let day_first = dates::day_first(&date_toks);

    // Pass 1: each row's date and raw values.
    struct Row {
        r: usize,
        date: Option<NaiveDate>,
        has_people: bool,
    }
    let mut rows: Vec<Row> = Vec::new();
    let mut section_date: Option<NaiveDate> = None;
    let mut section_anchor: Option<NaiveDate> = None;
    for r in start..g.height() {
        if g.row_nonempty(r) == 1 {
            // Section rows: "Week of 10/12" or "MONDAY 10/12".
            let c = g.first_nonempty(r).expect("one cell");
            let t = g.text(r, c);
            if let Some(a) = dates::week_anchor(t, ctx.today) {
                section_anchor = Some(a);
                continue;
            }
            if let Some(tok) = g.day(r, c) {
                section_date = resolve_tok(
                    ctx,
                    tok,
                    day_first,
                    section_anchor.or_else(|| ctx.anchor_for(r, 0..g.width())),
                );
                continue;
            }
        }
        if !is_data[r] {
            continue;
        }
        let anchor = section_anchor
            .or_else(|| ctx.anchor_for(r, 0..g.width()))
            .or(Some(ctx.default_anchor()));
        let mut date = None;
        if let Some(c) = date_col {
            if let Some(tok) = g.day(r, c) {
                date = resolve_tok(ctx, tok, day_first, anchor);
                if let (Some(w), Some(d)) = (tok.written_weekday().filter(|_| tok.has_date()), date)
                    && d.weekday() != w
                {
                    sink.ask(Question::WeekdayMismatch {
                        text: clean(g.text(r, c)),
                        used: d,
                    });
                }
            }
        } else if let Some(c) = day_col {
            if let Some(tok) = g.day(r, c) {
                date = resolve_tok(ctx, tok, day_first, anchor);
            }
        } else {
            date = section_date;
        }
        let has_people = roles
            .iter()
            .enumerate()
            .any(|(c, role)| *role == Role::Person && !g.is_empty(r, c));
        rows.push(Row {
            r,
            date,
            has_people,
        });
    }

    // Merged-cell blanks in the date column.
    if let Some(c) = date_col.or(day_col) {
        let mut labels: Vec<(usize, String, bool)> = rows
            .iter()
            .map(|row| {
                (
                    row.r,
                    row.date.map(|d| d.to_string()).unwrap_or_default(),
                    row.has_people,
                )
            })
            .collect();
        let header_text = header
            .map(|h| clean(g.text(h, c)))
            .unwrap_or_else(|| "Date".into());
        let block_row = g.source(rows[0].r, c).row;
        crate::layout::fill_down_labels(&mut labels, &header_text, &mut sink, block_row);
        for (row, (_, filled, _)) in rows.iter_mut().zip(labels) {
            if row.date.is_none() {
                row.date = NaiveDate::parse_from_str(&filled, "%Y-%m-%d").ok();
            }
        }
    }

    // Merged-cell blanks in the job column.
    let site_col = roles.iter().position(|r| *r == Role::Site);
    let mut site_fill: Vec<String> = Vec::new();
    if let Some(c) = site_col {
        // A row with a status ("off", "Out") has no job on purpose.
        let has_status =
            |r: usize| (0..g.width()).any(|cc| status_word(&key(g.text(r, cc))).is_some());
        let mut labels: Vec<(usize, String, bool)> = rows
            .iter()
            .map(|row| {
                (
                    row.r,
                    clean(g.text(row.r, c)),
                    row.has_people && !has_status(row.r),
                )
            })
            .collect();
        let header_text = header
            .map(|h| clean(g.text(h, c)))
            .unwrap_or_else(|| "Job".into());
        let block_row = g.source(rows[0].r, c).row;
        crate::layout::fill_down_labels(&mut labels, &header_text, &mut sink, block_row);
        site_fill = labels.into_iter().map(|(_, l, _)| l).collect();
    }

    // Merged-cell blanks in a name column: a name written once over its
    // rows. Only when that's the file's style (most rows blank under a name),
    // since a lone blank name can mean nobody is on the job.
    let mut person_fill: HashMap<(usize, usize), String> = HashMap::new();
    let person_cols: Vec<usize> = roles
        .iter()
        .enumerate()
        .filter(|(_, r)| **r == Role::Person)
        .map(|(c, _)| c)
        .collect();
    if let Some(&c) = person_cols.first() {
        // Rows with a date but no name in any name column.
        let with_data: Vec<&Row> = rows.iter().filter(|row| row.date.is_some()).collect();
        let nameless = |r: usize| person_cols.iter().all(|&pc| g.is_empty(r, pc));
        let blanks = with_data.iter().filter(|row| nameless(row.r)).count();
        if !with_data.is_empty() && blanks * 10 >= with_data.len() * 3 {
            let mut last = String::new();
            for row in &with_data {
                if nameless(row.r) {
                    if !last.is_empty() {
                        person_fill.insert((row.r, c), last.clone());
                    }
                } else if !g.is_empty(row.r, c) {
                    last = g.text(row.r, c).to_string();
                }
            }
        }
    }

    // Very large files: the first four weeks only.
    let mut keep_weeks: Option<Vec<NaiveDate>> = None;
    if g.height() > MAX_ROWS_BEFORE_CAP {
        let mut weeks: Vec<NaiveDate> = rows
            .iter()
            .filter_map(|row| row.date)
            .map(week_start)
            .collect();
        weeks.sort();
        weeks.dedup();
        if weeks.len() > 4 {
            weeks.truncate(4);
            sink.notes.push(format!(
                "This file has {} rows. Only the first 4 weeks found ({} to {}) were read.",
                g.height(),
                weeks[0].format("%-m/%-d/%Y"),
                (weeks[3] + Duration::days(6)).format("%-m/%-d/%Y")
            ));
            keep_weeks = Some(weeks);
        }
    }

    // Pass 2: records.
    for (i, row) in rows.iter().enumerate() {
        let r = row.r;
        let Some(date) = row.date else {
            sink.junk += 1;
            continue;
        };
        if keep_weeks
            .as_ref()
            .is_some_and(|w| !w.contains(&week_start(date)))
        {
            continue;
        }
        let site_text = site_col.map(|c| {
            if site_fill.is_empty() {
                clean(g.text(r, c))
            } else {
                site_fill[i].clone()
            }
        });
        read_row(
            ctx,
            &mut sink,
            &roles,
            r,
            date,
            site_text.as_deref(),
            &person_fill,
        );
    }

    // Window: every date from the earliest to the latest.
    if let (Some(first), Some(last)) = (
        sink.assignments.iter().map(|a| a.date).min(),
        sink.assignments.iter().map(|a| a.date).max(),
    ) {
        let mut d = first;
        while d <= last {
            sink.window.insert(d);
            d += Duration::days(1);
        }
    }
    for (c, role) in roles.iter().enumerate() {
        if *role == Role::Ignore && stats[c].n == 0 {
            continue;
        }
        let src = g.source(header.unwrap_or(0), c);
        sink.columns.push(ColumnReading {
            index: src.col,
            header: header.map(|h| clean(g.text(h, c))).unwrap_or_default(),
            role: role.label().into(),
        });
    }
    vec![Candidate {
        layout: Layout::LongList,
        arrangement: Arrangement::Single,
        sink,
    }]
}

fn week_start(d: NaiveDate) -> NaiveDate {
    d - Duration::days(d.weekday().num_days_from_monday() as i64)
}

fn resolve_tok(
    ctx: &SheetCtx,
    tok: DayTok,
    day_first: bool,
    anchor: Option<NaiveDate>,
) -> Option<NaiveDate> {
    match tok {
        DayTok::Weekday(_) | DayTok::Letter(_) => {
            let w = dates::weekday_sequence(&[tok])?;
            Some(dates::weekdays_in_week(&w, anchor.unwrap_or_else(|| ctx.default_anchor()))[0])
        }
        other => dates::resolve(&other, day_first, ctx.today),
    }
}

fn find_header(ctx: &SheetCtx) -> Option<usize> {
    let g = ctx.g;
    let mut best: Option<(usize, usize)> = None;
    for r in 0..g.height().min(25) {
        let hits = (0..g.width())
            .filter(|&c| header_role(&key(g.text(r, c))).is_some())
            .count();
        if hits >= 2 && best.is_none_or(|(h, _)| hits > h) {
            best = Some((hits, r));
        }
    }
    best.map(|(_, r)| r)
}

fn is_header_like(ctx: &SheetCtx, r: usize) -> bool {
    let g = ctx.g;
    (0..g.width())
        .filter(|&c| header_role(&key(g.text(r, c))).is_some())
        .count()
        >= 2
}

fn is_data_row(ctx: &SheetCtx, r: usize, header: Option<usize>) -> bool {
    let g = ctx.g;
    let n = g.row_nonempty(r);
    if n == 0 || (n == 1 && g.width() > 1) {
        return false;
    }
    if Some(r) != header && header.is_some() && is_header_like(ctx, r) {
        return false; // header repeated mid-file
    }
    let first = g.first_nonempty(r).map(|c| g.text(r, c)).unwrap_or("");
    let k = key(first);
    if is_total_label(&k) || is_roster_title(&k) || (n <= 2 && is_footer_text(first)) {
        return false;
    }
    if (0..g.width()).all(|c| g.is_empty(r, c) || ctx.consumed.contains(&(r, c))) {
        return false;
    }
    true
}

fn column_stats(ctx: &SheetCtx, c: usize, rows: &[usize]) -> Stats {
    let g = ctx.g;
    let mut s = Stats::default();
    for &r in rows {
        let t = g.text(r, c);
        if t.is_empty() || ctx.consumed.contains(&(r, c)) {
            continue;
        }
        s.n += 1;
        match g.day(r, c) {
            Some(DayTok::Weekday(_) | DayTok::Letter(_)) => s.weekday += 1,
            Some(DayTok::Serial(_)) => {
                // A serial only counts as a date in a typed date cell or when
                // the header says so; elsewhere it's just a number.
                if matches!(g.cell(r, c).value, Value::Number(_) | Value::Text) {
                    s.date += 1;
                }
            }
            Some(_) => s.date += 1,
            None => {}
        }
        if times::parse_range(t).is_some() {
            s.time += 1;
        }
        if times::parse_single(t).is_some() {
            s.single_time += 1;
            let lower = t.trim().to_ascii_lowercase();
            if lower.contains(':')
                || lower.ends_with('m')
                || lower.ends_with('a')
                || lower.ends_with('p')
            {
                s.clock += 1;
            }
        }
        let k = key(t);
        if status_word(&k).is_some() {
            s.status += 1;
        }
        if t.trim().parse::<f64>().is_ok() {
            s.numeric += 1;
        }
        if crew_name(&k) {
            s.crew += 1;
        }
        if status_word(&k).is_none() && !is_unstaffed_marker(&k) && g.day(r, c).is_none() {
            let people = cells::parse_cell(t, Expect::People);
            let p = if people.items.is_empty() {
                0.0
            } else {
                people
                    .items
                    .iter()
                    .map(|i| person_score(&i.text))
                    .sum::<f32>()
                    / people.items.len() as f32
            };
            s.person += p;
            s.site += site_score(t);
            s.entity_n += 1;
        }
    }
    s
}

fn assign_roles(
    ctx: &SheetCtx,
    header: Option<usize>,
    stats: &[Stats],
    sink: &mut Sink,
) -> Option<Vec<Role>> {
    let g = ctx.g;
    let w = stats.len();
    let hroles: Vec<Option<HeaderRole>> = (0..w)
        .map(|c| header.and_then(|h| header_role(&key(g.text(h, c)))))
        .collect();
    let rate = |x: usize, s: &Stats| if s.n == 0 { 0.0 } else { x as f32 / s.n as f32 };
    let mut roles = vec![Role::Ignore; w];

    // Dates.
    let date_col = (0..w)
        // One stray date isn't a date column, unless the header says so.
        .filter(|&c| {
            stats[c].date >= 2 || (stats[c].date >= 1 && hroles[c] == Some(HeaderRole::Date))
        })
        .filter(|&c| {
            rate(stats[c].date, &stats[c]) >= 0.5
                || (hroles[c] == Some(HeaderRole::Date) && rate(stats[c].date, &stats[c]) >= 0.3)
        })
        .max_by(|&a, &b| {
            let sa = rate(stats[a].date, &stats[a])
                + if hroles[a] == Some(HeaderRole::Date) {
                    1.0
                } else {
                    0.0
                };
            let sb = rate(stats[b].date, &stats[b])
                + if hroles[b] == Some(HeaderRole::Date) {
                    1.0
                } else {
                    0.0
                };
            sa.total_cmp(&sb)
        });
    if let Some(c) = date_col {
        roles[c] = Role::Date;
    }
    let day_col = (0..w)
        .filter(|&c| roles[c] == Role::Ignore && rate(stats[c].weekday, &stats[c]) >= 0.5)
        .max_by(|&a, &b| {
            rate(stats[a].weekday, &stats[a]).total_cmp(&rate(stats[b].weekday, &stats[b]))
        });
    if let Some(c) = day_col {
        if date_col.is_none() {
            roles[c] = Role::Day;
        } else {
            roles[c] = Role::Ignore; // a weekday next to a full date adds nothing
        }
    }

    for c in 0..w {
        if roles[c] != Role::Ignore || stats[c].n == 0 || Some(c) == day_col {
            continue;
        }
        let s = &stats[c];
        roles[c] = match hroles[c] {
            Some(HeaderRole::Time) if rate(s.time, s) >= 0.3 || s.n == 0 => Role::Time,
            Some(HeaderRole::Start) if rate(s.single_time, s) >= 0.5 => Role::Start,
            Some(HeaderRole::End) if rate(s.single_time, s) >= 0.5 => Role::End,
            Some(HeaderRole::Status) => Role::Status,
            Some(HeaderRole::Note) => Role::Note,
            Some(HeaderRole::Hours) if rate(s.time, s) >= 0.5 => Role::Time,
            // A header naming people, jobs or dates wins over content; those
            // roles are settled below.
            Some(_) => Role::Ignore,
            None if rate(s.time, s) >= 0.6 => Role::Time,
            None if rate(s.status, s) >= 0.9 => Role::Status,
            None => Role::Ignore,
        };
    }

    // Two untitled columns of clock times side by side: start and end.
    for c in 0..w.saturating_sub(1) {
        let single = |c: usize| {
            hroles[c].is_none()
                && roles[c] == Role::Ignore
                && stats[c].n > 0
                && rate(stats[c].clock, &stats[c]) >= 0.8
        };
        if single(c) && single(c + 1) {
            roles[c] = Role::Start;
            roles[c + 1] = Role::End;
        }
    }

    // People and sites among the remaining text columns.
    let mut person_cols = Vec::new();
    let mut roster_cols: Vec<usize> = Vec::new();
    let mut site_cands: Vec<(f32, usize)> = Vec::new();
    for c in 0..w {
        if roles[c] != Role::Ignore || stats[c].entity_n == 0 || Some(c) == day_col {
            continue;
        }
        if matches!(
            hroles[c],
            Some(HeaderRole::Hours) | Some(HeaderRole::JobNumber)
        ) {
            continue;
        }
        // A "Roster" column beside the list names the crew, not this row's
        // people, unless it's the list's only name column ("Team", "Employees").
        if header.is_some_and(|h| is_roster_title(&key(g.text(h, c)))) {
            roster_cols.push(c);
            continue;
        }
        let s = &stats[c];
        let mut p = s.person / s.entity_n as f32;
        let mut st = s.site / s.entity_n as f32;
        match hroles[c] {
            Some(HeaderRole::Person) | Some(HeaderRole::Crew) => p += 0.5,
            Some(HeaderRole::Site) => st += 0.5,
            _ => {}
        }
        if p > st && p >= 0.45 {
            person_cols.push(c);
        } else if st >= 0.35 {
            site_cands.push((st - p, c));
        }
        if header.is_none() && (p - st).abs() < 0.08 && p > 0.3 {
            let options = vec![
                Role::Person.label().to_string(),
                Role::Site.label().to_string(),
            ];
            let chosen = if p > st { Role::Person } else { Role::Site }
                .label()
                .to_string();
            sink.ask(Question::ColumnRole {
                header: format!("Column {}", g.source(0, c).col + 1),
                options,
                chosen,
            });
        }
    }
    if person_cols.is_empty() {
        person_cols = roster_cols
            .into_iter()
            .filter(|&c| stats[c].entity_n > 0 && stats[c].person / stats[c].entity_n as f32 >= 0.45)
            .collect();
    }
    if person_cols.is_empty() {
        return None;
    }
    for &c in &person_cols {
        roles[c] = Role::Person;
    }
    site_cands.sort_by(|a, b| b.0.total_cmp(&a.0));
    if let Some(&(_, c)) = site_cands.first() {
        roles[c] = Role::Site;
    } else if let Some(c) = (0..w).find(|&c| hroles[c] == Some(HeaderRole::JobNumber)) {
        roles[c] = Role::Site;
    }
    if !roles.contains(&Role::Date) && !roles.contains(&Role::Day) {
        // Dates may still come from section rows; need at least one.
        let any_section_date = (0..g.height()).any(|r| {
            g.row_nonempty(r) == 1 && g.first_nonempty(r).and_then(|c| g.day(r, c)).is_some()
        });
        if !any_section_date {
            return None;
        }
    }
    Some(roles)
}

fn read_row(
    ctx: &SheetCtx,
    sink: &mut Sink,
    roles: &[Role],
    r: usize,
    date: NaiveDate,
    site_text: Option<&str>,
    person_fill: &HashMap<(usize, usize), String>,
) {
    let g = ctx.g;
    let src_of = |c: usize| g.source(r, c);
    let mut extras = Extras::default();
    let (mut start, mut end) = (None, None);
    let mut status_from_col = None;
    for (c, role) in roles.iter().enumerate() {
        let t = g.text(r, c);
        if t.is_empty() {
            continue;
        }
        match role {
            Role::Time => extras.time = times::parse_range(t),
            Role::Start => start = Some(t.to_string()),
            Role::End => end = Some(t.to_string()),
            Role::Note => extras.note = Some(clean(t)),
            Role::Status => {
                let parsed = cells::parse_cell(t, Expect::Sites);
                status_from_col = parsed.status;
            }
            _ => {}
        }
    }
    if let (Some(s), Some(e)) = (&start, &end) {
        extras.time = times::from_pair(s, e);
    }

    let site_parsed = site_text.map(|t| cells::parse_cell(t, Expect::Sites));
    let mut people: Vec<(Item, usize)> = Vec::new();
    let mut unstaffed_at: Option<usize> = None;
    let mut person_status = None;
    for (c, role) in roles.iter().enumerate() {
        let text = person_fill
            .get(&(r, c))
            .map(String::as_str)
            .unwrap_or_else(|| g.text(r, c));
        if *role != Role::Person || text.trim().is_empty() {
            continue;
        }
        let parsed = cells::parse_cell(text, Expect::People);
        if parsed.unstaffed {
            unstaffed_at = Some(c);
        }
        if parsed.status.is_some() && parsed.items.is_empty() {
            person_status = parsed.status;
        }
        if extras.part.is_none() {
            extras.part = parsed.part;
        }
        if extras.time.is_none() {
            extras.time = parsed.time;
        }
        for item in parsed.items {
            people.push((item, c));
        }
    }

    let mut produced = false;
    if let Some(sp) = &site_parsed {
        let site_status = sp.status;
        let cell_extras = Extras {
            time: extras.time.or(sp.time),
            part: extras.part.or(sp.part),
            note: match (&sp.note, &extras.note) {
                (Some(a), Some(b)) => Some(format!("{a}; {b}")),
                (a, b) => a.clone().or_else(|| b.clone()),
            },
        };
        if let Some((status, part)) = site_status.filter(|_| sp.items.is_empty()) {
            for (item, c) in &people {
                sink.status(
                    &ctx.crews,
                    &item.text,
                    date,
                    (status, part.or(item.part)),
                    cell_extras.note.as_deref(),
                    src_of(*c),
                );
                produced = true;
            }
        } else {
            for site in &sp.items {
                for (item, c) in &people {
                    let merged = Item {
                        text: item.text.clone(),
                        note: match (&item.note, &site.note) {
                            (Some(a), Some(b)) => Some(format!("{a}; {b}")),
                            (a, b) => a.clone().or_else(|| b.clone()),
                        },
                        part: item.part.or(site.part),
                        time: item.time.or(site.time),
                    };
                    sink.work(
                        &ctx.crews,
                        &item.text,
                        &site.text,
                        date,
                        &merged,
                        &cell_extras,
                        src_of(*c),
                    );
                    produced = true;
                }
                if let Some(c) = unstaffed_at {
                    sink.unstaffed(&site.text, date, src_of(c));
                    produced = true;
                }
            }
        }
    }
    if let Some((status, part)) = status_from_col.or(person_status) {
        for (item, c) in &people {
            sink.status(
                &ctx.crews,
                &item.text,
                date,
                (status, part),
                extras.note.as_deref(),
                src_of(*c),
            );
            produced = true;
        }
    }
    if produced {
        sink.understood += 1
    } else {
        sink.junk += 1
    }
}
