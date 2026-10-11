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

    // Rows of day numbers under the weekday columns.
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
        return Vec::new();
    }

    let (year, month) = match month_title(ctx, header_row) {
        Some(ym) => ym,
        None => match infer_month(ctx.today, &weekdays, &number_rows[0].1) {
            Some(ym) => ym,
            None => return Vec::new(),
        },
    };

    // Dates for each (number row, column), rolling months forward when the
    // numbers restart.
    let first_n = number_rows[0]
        .1
        .iter()
        .flatten()
        .next()
        .copied()
        .unwrap_or(1);
    let mut offset: i32 = if first_n > 7 { -1 } else { 0 };
    let mut prev = 0u32;
    let mut cell_dates: Vec<Vec<Option<NaiveDate>>> = Vec::new();
    for (_, nums) in &number_rows {
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
        cell_dates.push(row_dates);
    }

    let mut sink = Sink::default();
    for (k, (r, _)) in number_rows.iter().enumerate() {
        let end = number_rows
            .get(k + 1)
            .map(|(nr, _)| *nr)
            .unwrap_or(g.height());
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
            for rr in r + 1..end {
                let t = g.text(rr, *c);
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
            sink.status(&ctx.crews, name, date, status, None, src);
            sink.understood += 1;
            return;
        }
        sink.junk += 1;
        return;
    };
    let (lk, rk) = (key(&left), key(&right));
    if let Some(status) = status_word(&rk) {
        for item in cells::parse_cell(&left, Expect::People).items {
            sink.status(&ctx.crews, &item.text, date, status, item.part, src);
        }
        sink.understood += 1;
        return;
    }
    if let Some(status) = status_word(&lk) {
        for item in cells::parse_cell(&right, Expect::People).items {
            sink.status(&ctx.crews, &item.text, date, status, item.part, src);
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
