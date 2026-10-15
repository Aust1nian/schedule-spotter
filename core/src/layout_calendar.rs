//! Month calendars: a row of weekday names, then for each week a row of day
//! numbers followed by entries like "Mike - Johnson" under each day.
//!
//! The month comes from a title ("October 2026"); without one, from the
//! month near today whose days line up under the weekday names.

use std::sync::LazyLock;

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use regex::Regex;

use crate::cells::{self, Expect, Item};
use crate::classify::{person_score, site_score};
use crate::dates::{self, DayTok};
use crate::layout::{Candidate, Extras, SheetCtx, Sink};
use crate::lexicon::{is_month_word, is_unstaffed_marker, status_word};
use crate::model::{Arrangement, ColumnReading, Layout};
use crate::read::Value;
use crate::text::{clean, key};

pub fn read(ctx: &SheetCtx) -> Vec<Candidate> {
    let g = ctx.g;
    let Some((header_row, cols)) = weekday_header(ctx) else {
        return Vec::new();
    };
    let weekdays: Vec<Weekday> = cols.iter().map(|(_, w)| *w).collect();

    // Rows of full dates ("10/18") under the weekday columns, if the
    // calendar writes them; otherwise rows of day numbers.
    let mut date_rows: Vec<(usize, Vec<Option<NaiveDate>>)> = Vec::new();
    for r in header_row + 1..g.height() {
        let toks: Vec<Option<DayTok>> = cols
            .iter()
            .map(|(c, _)| g.day(r, *c).filter(DayTok::has_date))
            .collect();
        let present: Vec<DayTok> = toks.iter().flatten().copied().collect();
        if present.len() < 3 {
            continue;
        }
        let df = dates::day_first(&present);
        let resolved: Vec<Option<NaiveDate>> = toks
            .iter()
            .map(|t| t.and_then(|t| dates::resolve(&t, df, ctx.today)))
            .collect();
        let days: Vec<NaiveDate> = resolved.iter().flatten().copied().collect();
        if days.windows(2).all(|w| (w[1] - w[0]).num_days() == 1) {
            date_rows.push((r, resolved));
        }
    }
    let (rows_at, cell_dates): (Vec<usize>, Vec<Vec<Option<NaiveDate>>>) = if date_rows.len() >= 2 {
        date_rows.into_iter().unzip()
    } else {
        match number_row_dates(ctx, header_row, &cols, &weekdays) {
            Some(found) => found,
            None => return Vec::new(),
        }
    };

    let mut sink = Sink::default();
    for (k, r) in rows_at.iter().enumerate() {
        let end = rows_at.get(k + 1).copied().unwrap_or(g.height());
        for (j, (c, w)) in cols.iter().enumerate() {
            let Some(date) = cell_dates[k][j] else {
                continue;
            };
            if date.weekday() != *w {
                sink.junk += 1;
            }
            // Entries: the day cell's own lines after the number, then the
            // cells below it until the next week's numbers.
            let mut texts: Vec<(String, usize)> = Vec::new();
            let own = g.text(*r, *c);
            if let Some((_, rest)) = own.split_once('\n') {
                texts.push((rest.to_string(), *r));
            }
            // Below the last week, a blank row ends the calendar: what follows
            // is notes ("Roof tear-off moved to 11/30 - Dana knows."), not
            // entries for the month's last days. A blank row is either kept
            // as an empty row or dropped by the grid, which shows as a gap
            // in the file's own row numbers.
            let last_week = k + 1 == rows_at.len();
            for rr in r + 1..end {
                if last_week
                    && (g.row_nonempty(rr) == 0
                        || g.source(rr, *c).row > g.source(rr - 1, *c).row + 1)
                {
                    break;
                }
                let t = g.text(rr, *c);
                // A sentence alone in its row under the last week, whose
                // "people" aren't names, is a note even where the file kept
                // no blank row before it. A long crew entry ("Alejandro A +
                // Lázaro R + Salvador H - Job 2612") still reads.
                if last_week
                    && g.row_nonempty(rr) == 1
                    && crate::layout::is_footer_text(t)
                    && entry_people_score(t) < 0.4
                {
                    break;
                }
                if !t.is_empty() && !ctx.consumed.contains(&(rr, *c)) {
                    texts.push((t.to_string(), rr));
                }
            }
            for (text, rr) in texts {
                for line in text.split('\n') {
                    read_entry(ctx, &mut sink, line, date, g.source(rr, *c));
                }
            }
        }
    }
    let dated: Vec<NaiveDate> = sink.assignments.iter().map(|a| a.date).collect();
    if let (Some(first), Some(last)) = (dated.iter().min(), dated.iter().max()) {
        let mut d = *first;
        while d <= *last {
            sink.window.insert(d);
            d += Duration::days(1);
        }
    }
    for (c, w) in &cols {
        sink.columns.push(ColumnReading {
            index: g.source(header_row, *c).col,
            header: clean(g.text(header_row, *c)),
            role: format!("{w}"),
        });
    }
    vec![Candidate {
        layout: Layout::CalendarMonth,
        arrangement: Arrangement::Single,
        sink,
    }]
}

/// Day-number rows ("13 14 15 …") and their dates: the month comes from a
/// title or from weekday alignment, rolling forward when numbers restart.
#[allow(clippy::type_complexity)]
fn number_row_dates(
    ctx: &SheetCtx,
    header_row: usize,
    cols: &[(usize, Weekday)],
    weekdays: &[Weekday],
) -> Option<(Vec<usize>, Vec<Vec<Option<NaiveDate>>>)> {
    let g = ctx.g;
    let mut number_rows: Vec<(usize, Vec<Option<u32>>)> = Vec::new();
    for r in header_row + 1..g.height() {
        let nums: Vec<Option<u32>> = cols
            .iter()
            .map(|(c, _)| leading_day_number(g, r, *c))
            .collect();
        let present: Vec<u32> = nums.iter().flatten().copied().collect();
        let increasing = present
            .windows(2)
            .all(|w| w[1] == w[0] + 1 || (w[1] == 1 && w[0] >= 28));
        if present.len() >= 3 && increasing {
            number_rows.push((r, nums));
        }
    }
    if number_rows.len() < 2 {
        return None;
    }
    // A month's first or last week can hold just one or two days ("29 | 30"
    // under Sun and Mon). Such a row counts when it holds nothing but those
    // numbers and continues the full weeks: the last week starts the next
    // day in the first column; the first week ends the day before in the
    // last column.
    let only_numbers = |r: usize, nums: &[Option<u32>]| {
        cols.iter()
            .zip(nums)
            .all(|((c, _), n)| n.is_some() == !g.is_empty(r, *c))
            && (0..g.width()).all(|c| g.is_empty(r, c) || cols.iter().any(|(cc, _)| *cc == c))
    };
    let short_nums = |r: usize| -> Option<Vec<Option<u32>>> {
        let nums: Vec<Option<u32>> = cols
            .iter()
            .map(|(c, _)| leading_day_number(g, r, *c))
            .collect();
        let n = nums.iter().flatten().count();
        let increasing = nums
            .iter()
            .flatten()
            .collect::<Vec<_>>()
            .windows(2)
            .all(|w| *w[1] == *w[0] + 1);
        ((1..3).contains(&n) && increasing && only_numbers(r, &nums)).then_some(nums)
    };
    let last_of = |nums: &[Option<u32>]| nums.iter().rev().flatten().next().copied();
    let first_of = |nums: &[Option<u32>]| nums.iter().flatten().next().copied();
    if let Some((last_r, last_nums)) = number_rows.last().cloned()
        && let Some(r) = (last_r + 1..g.height()).find(|&r| short_nums(r).is_some())
        && let Some(nums) = short_nums(r)
        && nums[0].is_some()
        && first_of(&nums) == last_of(&last_nums).map(|n| n + 1)
    {
        number_rows.push((r, nums));
    }
    if let Some((first_r, first_nums)) = number_rows.first().cloned()
        && let Some(r) = (header_row + 1..first_r)
            .rev()
            .find(|&r| short_nums(r).is_some())
        && let Some(nums) = short_nums(r)
        && nums[nums.len() - 1].is_some()
        && first_of(&first_nums) == last_of(&nums).map(|n| n + 1)
    {
        number_rows.insert(0, (r, nums));
    }
    let (year, month) = match month_title(ctx, header_row) {
        Some(ym) => ym,
        None => infer_month(ctx.today, weekdays, &number_rows[0].1)?,
    };
    // The first week may start in the titled month (a "Sep – Oct" calendar
    // that opens on the 13th) or show the previous month's last days: pick
    // the month that puts the first numbers under the right weekday names.
    let first_cells: Vec<(u32, Weekday)> = number_rows[0]
        .1
        .iter()
        .zip(weekdays)
        .filter_map(|(n, w)| n.map(|n| (n, *w)))
        .collect();
    let fits = |offset: i32| {
        let total = year * 12 + (month as i32 - 1) + offset;
        let (y, m) = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32);
        let mut prev = 0;
        first_cells
            .iter()
            .take_while(|(n, _)| {
                let ok = *n > prev;
                prev = *n;
                ok
            })
            .all(|(n, w)| NaiveDate::from_ymd_opt(y, m, *n).is_some_and(|d| d.weekday() == *w))
    };
    let mut offset: i32 = [0, -1, 1].into_iter().find(|o| fits(*o)).unwrap_or(0);
    let mut prev = 0u32;
    let mut rows = Vec::new();
    let mut cell_dates = Vec::new();
    for (r, nums) in &number_rows {
        let mut row_dates = Vec::new();
        for n in nums {
            row_dates.push(n.and_then(|n| {
                if n < prev {
                    offset += 1;
                }
                prev = n;
                let total = year * 12 + (month as i32 - 1) + offset;
                NaiveDate::from_ymd_opt(total.div_euclid(12), (total.rem_euclid(12) + 1) as u32, n)
            }));
        }
        rows.push(*r);
        cell_dates.push(row_dates);
    }
    Some((rows, cell_dates))
}

/// A row with five or more weekday names in order and no dates.
fn weekday_header(ctx: &SheetCtx) -> Option<(usize, Vec<(usize, Weekday)>)> {
    let g = ctx.g;
    for r in 0..g.height() {
        let cells: Vec<(usize, DayTok)> = (0..g.width())
            .filter_map(|c| g.day(r, c).map(|t| (c, t)))
            .collect();
        if cells.len() < 5 || cells.iter().any(|(_, t)| !matches!(t, DayTok::Weekday(_))) {
            continue;
        }
        let toks: Vec<DayTok> = cells.iter().map(|(_, t)| *t).collect();
        let Some(seq) = dates::weekday_sequence(&toks) else {
            continue;
        };
        return Some((r, cells.iter().map(|(c, _)| *c).zip(seq).collect()));
    }
    None
}

fn leading_day_number(g: &crate::grid::Grid, r: usize, c: usize) -> Option<u32> {
    let cell = g.cell(r, c);
    if let Value::Number(n) = cell.value {
        return (n.fract() == 0.0 && (1.0..=31.0).contains(&n)).then_some(n as u32);
    }
    let first = cell.text.split('\n').next()?.trim();
    let n: u32 = first.parse().ok()?;
    (1..=31).contains(&n).then_some(n)
}

static MONTH_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b([a-záéíóú]{3,10})\.?\s*,?\s*(\d{4})?\b").expect("regex"));

/// "October 2026" or "OCT" somewhere above the weekday names.
fn month_title(ctx: &SheetCtx, header_row: usize) -> Option<(i32, u32)> {
    let g = ctx.g;
    let mut sources: Vec<String> = (0..header_row)
        .rev()
        .flat_map(|r| (0..g.width()).map(move |c| (r, c)))
        .map(|(r, c)| g.text(r, c).to_string())
        .collect();
    sources.push(g.name.clone());
    for text in sources {
        let k = key(&text);
        for cap in MONTH_TITLE.captures_iter(&k) {
            if let Some(m) = is_month_word(&cap[1]) {
                let year = cap.get(2).and_then(|y| y.as_str().parse::<i32>().ok());
                let year = year.unwrap_or_else(|| {
                    dates::closest_year(m, 15, ctx.today)
                        .map(|d| d.year())
                        .unwrap_or(ctx.today.year())
                });
                return Some((year, m));
            }
        }
    }
    None
}

/// The month within a year of today whose day numbers fall under the right
/// weekday names.
fn infer_month(
    today: NaiveDate,
    weekdays: &[Weekday],
    first_row: &[Option<u32>],
) -> Option<(i32, u32)> {
    let mut best: Option<(i64, (i32, u32))> = None;
    for delta in -12i32..=12 {
        let total = today.year() * 12 + today.month() as i32 - 1 + delta;
        let (y, m) = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32);
        let fits = first_row.iter().zip(weekdays).all(|(n, w)| match n {
            Some(n) if *n <= 7 => {
                NaiveDate::from_ymd_opt(y, m, *n).is_some_and(|d| d.weekday() == *w)
            }
            _ => true,
        });
        if fits {
            let d = NaiveDate::from_ymd_opt(y, m, 15)?;
            let dist = (d - today).num_days().abs();
            if best.is_none_or(|(b, _)| dist < b) {
                best = Some((dist, (y, m)));
            }
        }
    }
    best.map(|(_, ym)| ym)
}

const SEPARATORS: [&str; 9] = [
    " @ ", " at ", ": ", " - ", " – ", " — ", " -> ", " → ", " = ",
];

/// One calendar line: "Mike - Johnson", "Johnson: Mike, Jose", "Luis OFF".
/// How much the people side of an entry line looks like names, split and
/// chosen the way `read_entry` does it (0 when there's no "people - job").
fn entry_people_score(line: &str) -> f32 {
    let line = clean(line);
    let Some((left, right)) = SEPARATORS.iter().find_map(|sep| line.split_once(sep)) else {
        return 0.0;
    };
    let people_score = |t: &str| {
        let items = cells::parse_cell(t, Expect::People).items;
        if items.is_empty() {
            0.0
        } else {
            items.iter().map(|i| person_score(&i.text)).sum::<f32>() / items.len() as f32
        }
    };
    let (left, right) = (left.trim(), right.trim());
    if people_score(left) + site_score(right) >= people_score(right) + site_score(left) {
        people_score(left)
    } else {
        people_score(right)
    }
}

fn read_entry(
    ctx: &SheetCtx,
    sink: &mut Sink,
    line: &str,
    date: NaiveDate,
    src: crate::model::CellRef,
) {
    let line = clean(line);
    if line.is_empty() {
        return;
    }
    let split = SEPARATORS.iter().find_map(|sep| {
        line.split_once(sep)
            .map(|(a, b)| (a.trim().to_string(), b.trim().to_string()))
    });
    let Some((left, right)) = split else {
        // "Luis OFF" / "Luis - " with a trailing status word.
        if let Some((name, last)) = line.rsplit_once(' ')
            && let Some(status) = status_word(&key(last))
        {
            sink.status(&ctx.crews, name, date, (status, None), None, src);
            sink.understood += 1;
            return;
        }
        sink.junk += 1;
        return;
    };
    let (lk, rk) = (key(&left), key(&right));
    if let Some(status) = status_word(&rk) {
        for item in cells::parse_cell(&left, Expect::People).items {
            sink.status(
                &ctx.crews,
                &item.text,
                date,
                (status, item.part),
                item.note.as_deref(),
                src,
            );
        }
        sink.understood += 1;
        return;
    }
    if let Some(status) = status_word(&lk) {
        for item in cells::parse_cell(&right, Expect::People).items {
            sink.status(
                &ctx.crews,
                &item.text,
                date,
                (status, item.part),
                item.note.as_deref(),
                src,
            );
        }
        sink.understood += 1;
        return;
    }
    if is_unstaffed_marker(&rk) {
        sink.unstaffed(&left, date, src);
        sink.understood += 1;
        return;
    }
    if is_unstaffed_marker(&lk) {
        sink.unstaffed(&right, date, src);
        sink.understood += 1;
        return;
    }
    let people_score = |t: &str| {
        let items = cells::parse_cell(t, Expect::People).items;
        if items.is_empty() {
            0.0
        } else {
            items.iter().map(|i| person_score(&i.text)).sum::<f32>() / items.len() as f32
        }
    };
    let left_people =
        people_score(&left) + site_score(&right) >= people_score(&right) + site_score(&left);
    let (people_text, site_text) = if left_people {
        (left, right)
    } else {
        (right, left)
    };
    let people = cells::parse_cell(&people_text, Expect::People);
    let sites = cells::parse_cell(&site_text, Expect::Sites);
    let extras = Extras {
        time: people.time.or(sites.time),
        part: people.part.or(sites.part),
        note: sites.note.clone().or(people.note.clone()),
    };
    let mut any = false;
    for site in &sites.items {
        for p in &people.items {
            let item = Item {
                text: p.text.clone(),
                note: p.note.clone().or(site.note.clone()),
                part: p.part.or(site.part),
                time: p.time.or(site.time),
            };
            sink.work(&ctx.crews, &p.text, &site.text, date, &item, &extras, src);
            any = true;
        }
    }
    if any {
        sink.understood += 1
    } else {
        sink.junk += 1
    }
}
