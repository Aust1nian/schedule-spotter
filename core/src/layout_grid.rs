//! Grids with a day axis: people or jobs down the side and days across the
//! top. Day × job and day × person grids are the same thing transposed, and
//! are read by running this reader on the transposed sheet.
//!
//! Several day headers in different rows are stacked weeks; several in one
//! row are side-by-side weeks.

use chrono::NaiveDate;

use crate::cells::{self, Expect};
use crate::classify::{person_score, site_score};
use crate::dates::{self, DayTok};
use crate::layout::{
    Candidate, Extras, SheetCtx, Sink, fill_down_labels, is_footer_text, job_cell, person_cell,
};
use crate::lexicon::{
    crew_name, is_crew_roster_title, is_roster_title, is_total_label, status_word,
};
use crate::model::{Arrangement, ColumnReading, Layout, Part, Question};
use crate::text::{clean, key};
use crate::times;

#[derive(Clone, Debug)]
struct Run {
    row: usize,
    cols: Vec<usize>,
    toks: Vec<DayTok>,
    dates: Vec<NaiveDate>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SubRole {
    Main,
    Part(Part),
    Time,
    Start,
    End,
    Note,
}

#[derive(Clone, Debug)]
struct DaySpan {
    date: NaiveDate,
    cols: Vec<(usize, SubRole)>,
}

#[derive(Clone, Debug)]
struct Section {
    header_row: usize,
    label_col: Option<usize>,
    days: Vec<DaySpan>,
    body: std::ops::Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    People,
    Jobs,
}

pub fn read(ctx: &SheetCtx, transposed: bool) -> Vec<Candidate> {
    let runs = find_runs(ctx);
    if runs.is_empty() {
        return Vec::new();
    }
    let sections = build_sections(ctx, &runs);
    if sections.is_empty() {
        return Vec::new();
    }
    let kind = decide_kind(ctx, &sections);
    let mut out = Vec::new();
    // Read it the likelier way first; the other way only competes when the
    // evidence is close, so a coin flip gets asked about.
    let (first, margin) = kind;
    out.push(extract(ctx, &runs, &sections, first, transposed));
    if margin < 0.15 {
        let other = if first == Kind::People {
            Kind::Jobs
        } else {
            Kind::People
        };
        let mut alt = extract(ctx, &runs, &sections, other, transposed);
        // Slight handicap so the evidence-based reading wins ties.
        alt.sink.junk += 1;
        out.push(alt);
    }
    out
}

fn find_runs(ctx: &SheetCtx) -> Vec<Run> {
    let g = ctx.g;
    let mut runs = Vec::new();
    for r in 0..g.height() {
        let mut groups: Vec<Vec<(usize, DayTok)>> = Vec::new();
        let mut cur: Vec<(usize, DayTok)> = Vec::new();
        for c in 0..g.width() {
            if ctx.consumed.contains(&(r, c)) {
                if !cur.is_empty() {
                    groups.push(std::mem::take(&mut cur));
                }
                continue;
            }
            match g.day(r, c) {
                Some(tok) => {
                    let weekday_only =
                        |t: &DayTok| matches!(t, DayTok::Weekday(_) | DayTok::Letter(_));
                    let mixes = cur
                        .last()
                        .is_some_and(|(_, prev)| weekday_only(prev) != weekday_only(&tok));
                    let serial_mix = cur.last().is_some_and(|(_, prev)| {
                        matches!(prev, DayTok::Serial(_)) != matches!(tok, DayTok::Serial(_))
                    });
                    let wraps = weekday_only(&tok) && !cur.is_empty() && {
                        let mut toks: Vec<DayTok> = cur.iter().map(|(_, t)| *t).collect();
                        toks.push(tok);
                        dates::weekday_sequence(&toks).is_none()
                    };
                    if mixes || serial_mix || wraps {
                        groups.push(std::mem::take(&mut cur));
                    }
                    cur.push((c, tok));
                }
                None if g.is_empty(r, c) => {}
                None => {
                    if !cur.is_empty() {
                        groups.push(std::mem::take(&mut cur));
                    }
                }
            }
        }
        if !cur.is_empty() {
            groups.push(cur);
        }
        for group in groups {
            if group.len() < 2 {
                continue;
            }
            let toks: Vec<DayTok> = group.iter().map(|(_, t)| *t).collect();
            // A run of bare single letters must look like a header, not data.
            if toks.iter().all(|t| matches!(t, DayTok::Letter(_))) && group.len() < 3 {
                continue;
            }
            let cols: Vec<usize> = group.iter().map(|(c, _)| *c).collect();
            let span = cols[0]..cols[cols.len() - 1] + 1;
            let anchor = ctx.anchor_for(r, span).or(Some(ctx.default_anchor()));
            let Some(dates) = dates::resolve_run(&toks, anchor, ctx.today) else {
                continue;
            };
            if dates.windows(2).any(|w| w[1] <= w[0])
                || (dates[dates.len() - 1] - dates[0]).num_days() > 62
            {
                continue;
            }
            runs.push(Run {
                row: r,
                cols,
                toks,
                dates,
            });
        }
    }
    // Weekday-only weeks that share one "Week of" anchor follow each other:
    // a later run (to the right, or below) can't repeat an earlier week.
    for i in 1..runs.len() {
        let weekday_only = runs[i]
            .toks
            .iter()
            .all(|t| matches!(t, DayTok::Weekday(_) | DayTok::Letter(_)));
        if !weekday_only {
            continue;
        }
        let prev_last = runs[..i]
            .iter()
            .map(|o| o.dates[o.dates.len() - 1])
            .max()
            .expect("non-empty");
        while runs[i].dates[0] <= prev_last {
            for d in &mut runs[i].dates {
                *d += chrono::Duration::days(7);
            }
        }
    }
    runs
}

fn build_sections(ctx: &SheetCtx, runs: &[Run]) -> Vec<Section> {
    let g = ctx.g;
    let mut sections: Vec<Section> = Vec::new();
    for (i, run) in runs.iter().enumerate() {
        let prev_same_row = runs[..i].iter().rev().find(|o| o.row == run.row);
        let next_same_row = runs[i + 1..].iter().find(|o| o.row == run.row);
        let lo = prev_same_row
            .map(|p| p.cols[p.cols.len() - 1] + 1)
            .unwrap_or(0);
        let hi = next_same_row.map(|n| n.cols[0]).unwrap_or(g.width());
        let first_day = run.cols[0];
        let end_row = runs
            .iter()
            .filter(|o| o.row > run.row && o.cols[0] < hi && o.cols[o.cols.len() - 1] >= lo)
            .map(|o| o.row)
            .min()
            .unwrap_or(g.height());

        // Two-row header: "Monday" over "AM | PM".
        let sub_row = (run.row + 1 < end_row && is_sub_header_row(g, run.row + 1, run.cols[0], hi))
            .then_some(run.row + 1);
        let body_start = sub_row.map(|r| r + 1).unwrap_or(run.row + 1);
        let body = body_start..end_row;

        let label_col = pick_label_col(ctx, lo, first_day, body.clone()).or_else(|| {
            sections
                .iter()
                .rev()
                .find(|s| s.header_row == run.row)
                .and_then(|s| s.label_col)
        });

        // Day spans: from each day's column to the next day's.
        let typical = if run.cols.len() >= 2 {
            let mut diffs: Vec<usize> = run.cols.windows(2).map(|w| w[1] - w[0]).collect();
            diffs.sort_unstable();
            diffs[diffs.len() / 2]
        } else {
            1
        };
        let mut days = Vec::new();
        for (k, &c) in run.cols.iter().enumerate() {
            let end = match run.cols.get(k + 1) {
                Some(&n) => n,
                None => {
                    let mut e = c + 1;
                    while e < hi.min(c + typical) && g.is_empty(run.row, e) {
                        e += 1;
                    }
                    e
                }
            };
            let cols: Vec<(usize, SubRole)> = (c..end)
                .filter(|&cc| label_col != Some(cc))
                .enumerate()
                .map(|(j, cc)| (cc, sub_role(ctx, sub_row, cc, j, body.clone())))
                .collect();
            days.push(DaySpan {
                date: run.dates[k],
                cols,
            });
        }
        sections.push(Section {
            header_row: run.row,
            label_col,
            days,
            body,
        });
    }
    sections
}

fn is_sub_header_row(g: &crate::grid::Grid, r: usize, from: usize, to: usize) -> bool {
    let mut words = 0;
    let mut other = 0;
    for c in from..to.min(g.width()) {
        let t = g.text(r, c);
        if t.is_empty() {
            continue;
        }
        if sub_header_word(&key(t)).is_some() {
            words += 1;
        } else {
            other += 1;
        }
    }
    words >= 2 && other == 0
}

fn sub_header_word(k: &str) -> Option<SubRole> {
    Some(match k {
        "am" | "a.m." | "morning" | "manana" => SubRole::Part(Part::Am),
        "pm" | "p.m." | "afternoon" | "tarde" => SubRole::Part(Part::Pm),
        "job" | "site" | "location" | "where" | "obra" | "project" | "jobsite" | "crew" | "who"
        | "names" | "name" => SubRole::Main,
        "hours" | "hrs" | "time" | "shift" | "horario" | "hora" => SubRole::Time,
        "start" | "in" | "entrada" => SubRole::Start,
        "end" | "out" | "salida" => SubRole::End,
        "notes" | "note" | "task" | "notas" => SubRole::Note,
        _ => return None,
    })
}

fn sub_role(
    ctx: &SheetCtx,
    sub_row: Option<usize>,
    c: usize,
    index_in_span: usize,
    body: std::ops::Range<usize>,
) -> SubRole {
    if let Some(r) = sub_row
        && let Some(role) = sub_header_word(&key(ctx.g.text(r, c)))
    {
        return role;
    }
    if index_in_span == 0 {
        return SubRole::Main;
    }
    // Untitled extra column: times if it mostly holds times.
    let values: Vec<&str> = body
        .map(|r| ctx.g.text(r, c))
        .filter(|t| !t.is_empty())
        .collect();
    let timeish = values
        .iter()
        .filter(|t| times::parse_range(t).is_some())
        .count();
    if !values.is_empty() && timeish * 10 >= values.len() * 6 {
        SubRole::Time
    } else {
        SubRole::Main
    }
}

fn pick_label_col(
    ctx: &SheetCtx,
    lo: usize,
    first_day: usize,
    body: std::ops::Range<usize>,
) -> Option<usize> {
    let g = ctx.g;
    let mut best: Option<(f32, usize)> = None;
    for c in lo..first_day {
        let values: Vec<&str> = body
            .clone()
            .filter(|&r| !ctx.consumed.contains(&(r, c)))
            .map(|r| g.text(r, c))
            .filter(|t| !t.is_empty() && !is_total_label(&key(t)))
            .collect();
        if values.is_empty() {
            continue;
        }
        let entity: f32 = values
            .iter()
            .map(|t| person_score(t).max(site_score(t)))
            .sum::<f32>()
            / values.len() as f32;
        // Closer to the days wins ties ("Name | Phone | Mon").
        let score = values.len() as f32 * entity + (c as f32) * 0.001;
        if best.is_none_or(|(s, _)| score > s) {
            best = Some((score, c));
        }
    }
    best.map(|(_, c)| c)
}

/// Whether a label marks a row (or the rest of the block) as not schedule.
fn skip_label(text: &str) -> Option<bool> {
    let k = key(text);
    if is_total_label(&k) {
        return Some(false); // skip this row
    }
    if is_roster_title(&k) || is_crew_roster_title(&k) {
        return Some(true); // stop the section here
    }
    if is_footer_text(text) {
        return Some(false);
    }
    None
}

fn label_status(text: &str) -> Option<crate::model::Status> {
    let k = key(text);
    status_word(&k).or_else(|| {
        k.split(['/', ',', '&'])
            .next()
            .and_then(|first| status_word(first.trim()))
    })
}

/// (kind, margin): which way the evidence points, and how strongly.
fn decide_kind(ctx: &SheetCtx, sections: &[Section]) -> (Kind, f32) {
    let g = ctx.g;
    let (mut lp, mut ls, mut ln) = (0.0f32, 0.0f32, 0usize);
    let mut status_labels = 0usize;
    let (mut bp, mut bpn, mut bs, mut bsn) = (0.0f32, 0usize, 0.0f32, 0usize);
    let (mut status_cells, mut multi_cells, mut unstaffed_cells, mut cells_n) =
        (0usize, 0usize, 0usize, 0usize);
    for s in sections {
        for r in s.body.clone() {
            if let Some(c) = s.label_col {
                let t = g.text(r, c);
                if !t.is_empty() && skip_label(t).is_none() && !ctx.consumed.contains(&(r, c)) {
                    if label_status(t).is_some() {
                        status_labels += 1;
                    } else {
                        lp += person_score(t);
                        ls += site_score(t);
                        ln += 1;
                    }
                }
            }
            for day in &s.days {
                for &(c, role) in &day.cols {
                    if !matches!(role, SubRole::Main | SubRole::Part(_)) {
                        continue;
                    }
                    let t = g.text(r, c);
                    if t.is_empty() || ctx.consumed.contains(&(r, c)) {
                        continue;
                    }
                    cells_n += 1;
                    let pp = cells::parse_cell(t, Expect::People);
                    let ps = cells::parse_cell(t, Expect::Sites);
                    if ps.status.is_some() && ps.items.is_empty() {
                        status_cells += 1;
                    }
                    if pp.unstaffed {
                        unstaffed_cells += 1;
                    }
                    if pp.items.len() >= 2 && pp.items.iter().all(|i| person_score(&i.text) >= 0.5)
                    {
                        multi_cells += 1;
                    }
                    if !pp.items.is_empty() {
                        bp += pp.items.iter().map(|i| person_score(&i.text)).sum::<f32>()
                            / pp.items.len() as f32;
                        bpn += 1;
                    }
                    if !ps.items.is_empty() {
                        bs += ps.items.iter().map(|i| site_score(&i.text)).sum::<f32>()
                            / ps.items.len() as f32;
                        bsn += 1;
                    }
                }
            }
        }
    }
    let avg = |s: f32, n: usize| if n == 0 { 0.0 } else { s / n as f32 };
    let frac = |x: usize| {
        if cells_n == 0 {
            0.0
        } else {
            x as f32 / cells_n as f32
        }
    };
    let people = avg(lp, ln) + avg(bs, bsn) + 0.5 * frac(status_cells);
    let jobs = avg(ls, ln)
        + avg(bp, bpn)
        + 0.5 * (frac(multi_cells) + frac(unstaffed_cells))
        + if status_labels > 0 { 0.3 } else { 0.0 };
    if people >= jobs {
        (Kind::People, people - jobs)
    } else {
        (Kind::Jobs, jobs - people)
    }
}

fn extract(
    ctx: &SheetCtx,
    runs: &[Run],
    sections: &[Section],
    kind: Kind,
    transposed: bool,
) -> Candidate {
    let g = ctx.g;
    let mut sink = Sink::default();

    // Weekday written next to a date that disagrees with it.
    for run in runs {
        for (k, tok) in run.toks.iter().enumerate() {
            if let Some(w) = tok.written_weekday().filter(|_| tok.has_date()) {
                use chrono::Datelike;
                if run.dates[k].weekday() != w {
                    let c = run.cols[k];
                    sink.ask(Question::WeekdayMismatch {
                        text: clean(g.text(run.row, c)),
                        used: run.dates[k],
                    });
                }
            }
        }
    }

    let crew_rows = kind == Kind::People && !ctx.crews.is_empty();
    for s in sections {
        for d in &s.days {
            sink.window.insert(d.date);
        }
        let label_header = s
            .label_col
            .map(|c| clean(g.text(s.header_row, c)))
            .unwrap_or_default();
        // Labels with merged-cell blanks filled.
        let mut labels: Vec<(usize, String, bool)> = Vec::new();
        for r in s.body.clone() {
            let label = s
                .label_col
                .map(|c| g.text(r, c).to_string())
                .unwrap_or_default();
            if let Some(stop) = skip_label(&label) {
                if stop {
                    break;
                }
                continue;
            }
            if s.label_col.is_some_and(|c| ctx.consumed.contains(&(r, c))) {
                continue;
            }
            let has_data = s.days.iter().any(|d| {
                d.cols
                    .iter()
                    .any(|&(c, _)| !g.is_empty(r, c) && !ctx.consumed.contains(&(r, c)))
            });
            if label.trim().is_empty() && !has_data {
                continue;
            }
            labels.push((r, clean(&label), has_data));
        }
        let block_row = g.source(s.header_row, 0).row;
        fill_down_labels(&mut labels, &label_header, &mut sink, block_row);

        for (r, label, _) in labels {
            if label.is_empty() {
                // Data with no label and nothing to fill from.
                sink.junk += 1;
                continue;
            }
            let status_row = (kind == Kind::Jobs).then(|| label_status(&label)).flatten();
            if kind == Kind::Jobs && status_row.is_none() {
                sink.grid_job(&label);
            }
            if kind == Kind::People && crew_name(&key(&label)) && !crew_rows {
                // A crew label in a person grid with no crew list to expand it.
                sink.notes.push(format!(
                    "No crew list found for \u{201c}{label}\u{201d}; read it as one name."
                ));
            }
            for day in &s.days {
                read_day_cell(ctx, &mut sink, kind, status_row, &label, r, day);
            }
        }
    }

    let layout = match (kind, transposed) {
        (Kind::People, false) => Layout::PersonByDay,
        (Kind::People, true) => Layout::DayByPerson,
        (Kind::Jobs, false) => Layout::JobByDay,
        (Kind::Jobs, true) => Layout::DayByJob,
    };
    let arrangement = arrangement_of(runs);
    if let Some(first) = sections.first() {
        if let Some(c) = first.label_col {
            let src = g.source(first.header_row, c);
            sink.columns.push(ColumnReading {
                index: if transposed { src.row } else { src.col },
                header: clean(g.text(first.header_row, c)),
                role: if kind == Kind::People {
                    "Person".into()
                } else {
                    "Job".into()
                },
            });
        }
        for d in &first.days {
            if let Some(&(c, _)) = d.cols.first() {
                let src = g.source(first.header_row, c);
                sink.columns.push(ColumnReading {
                    index: if transposed { src.row } else { src.col },
                    header: clean(g.text(first.header_row, c)),
                    role: format!("Day {}", d.date.format("%a %-m/%-d")),
                });
            }
        }
    }
    Candidate {
        layout,
        arrangement,
        sink,
    }
}

fn read_day_cell(
    ctx: &SheetCtx,
    sink: &mut Sink,
    kind: Kind,
    status_row: Option<crate::model::Status>,
    label: &str,
    r: usize,
    day: &DaySpan,
) {
    let g = ctx.g;
    // Gather the span's extra columns (time, start/end, notes) first.
    let mut extras = Extras::default();
    let (mut start, mut end) = (None, None);
    for &(c, role) in &day.cols {
        let t = g.text(r, c);
        if t.is_empty() {
            continue;
        }
        match role {
            SubRole::Time => {
                extras.time = times::parse_range(t).or(extras.time);
                sink.understood += 1;
            }
            SubRole::Start => start = Some(t.to_string()),
            SubRole::End => end = Some(t.to_string()),
            SubRole::Note => {
                extras.note = Some(clean(t));
                sink.understood += 1;
            }
            _ => {}
        }
    }
    if let (Some(s), Some(e)) = (&start, &end) {
        extras.time = times::from_pair(s, e).or(extras.time);
        sink.understood += 1;
    }

    // Each AM or PM sub-column is its own record, as written.
    let mains: Vec<(usize, Option<Part>)> = day
        .cols
        .iter()
        .filter_map(|&(c, role)| match role {
            SubRole::Main => Some((c, None)),
            SubRole::Part(p) => Some((c, Some(p))),
            _ => None,
        })
        .collect();
    for &(c, part) in &mains {
        if ctx.consumed.contains(&(r, c)) {
            continue;
        }
        let t = g.text(r, c);
        if t.is_empty() {
            continue;
        }
        let src = g.source(r, c);
        let cell_extras = Extras {
            part,
            ..extras.clone()
        };
        match kind {
            Kind::People => {
                let parsed = cells::parse_cell(t, Expect::Sites);
                person_cell(
                    sink,
                    &ctx.crews,
                    label,
                    day.date,
                    &parsed,
                    &cell_extras,
                    src,
                );
            }
            Kind::Jobs => {
                let parsed = cells::parse_cell(t, Expect::People);
                if let Some(status) = status_row {
                    for item in &parsed.items {
                        sink.status(
                            &ctx.crews,
                            &item.text,
                            day.date,
                            (status, item.part.or(cell_extras.part)),
                            item.note.as_deref(),
                            src,
                        );
                    }
                    if parsed.items.is_empty() {
                        sink.junk += 1
                    } else {
                        sink.understood += 1
                    }
                } else {
                    job_cell(
                        sink,
                        &ctx.crews,
                        label,
                        day.date,
                        &parsed,
                        &cell_extras,
                        src,
                    );
                }
            }
        }
    }
}

/// Weeks one under another are stacked; more than one week in a header row
/// is side by side.
fn arrangement_of(runs: &[Run]) -> Arrangement {
    use chrono::Datelike;
    let rows: std::collections::BTreeSet<usize> = runs.iter().map(|r| r.row).collect();
    let weeks_in_a_row = rows.iter().any(|row| {
        let weeks: std::collections::BTreeSet<_> = runs
            .iter()
            .filter(|r| r.row == *row)
            .flat_map(|r| r.dates.iter().map(|d| d.iso_week()))
            .collect();
        weeks.len() > 1
    });
    if weeks_in_a_row {
        Arrangement::SideBySide
    } else if rows.len() > 1 {
        Arrangement::Stacked
    } else {
        Arrangement::Single
    }
}
