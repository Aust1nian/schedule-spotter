//! Stage 5: dates and days.
//!
//! Single cells parse to a [`DayTok`], which may still be ambiguous (10/12
//! could be October 12 or 10 December; "T" could be Tuesday or Thursday).
//! Ambiguity is settled over a whole column or header run: month-first unless
//! a value over 12 appears first, the year closest to today, letters read in
//! sequence, weekday names anchored by a "Week of" date.

use std::sync::LazyLock;

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use regex::Regex;

use crate::lexicon::is_month_word;
use crate::text::key;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Letter {
    M,
    T,
    W,
    Th,
    F,
    S,
    Sa,
    Su,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DayTok {
    /// Month and day known (month written as a word, or ISO).
    Ymd {
        year: Option<i32>,
        month: u32,
        day: u32,
        weekday: Option<Weekday>,
    },
    /// Two numbers whose order is settled per column: `first/second`.
    Numeric {
        first: u32,
        second: u32,
        year: Option<i32>,
        weekday: Option<Weekday>,
    },
    Weekday(Weekday),
    Letter(Letter),
    /// An Excel serial day number.
    Serial(i64),
}

impl DayTok {
    pub fn written_weekday(&self) -> Option<Weekday> {
        match self {
            DayTok::Ymd { weekday, .. } | DayTok::Numeric { weekday, .. } => *weekday,
            DayTok::Weekday(w) => Some(*w),
            _ => None,
        }
    }

    pub fn has_date(&self) -> bool {
        matches!(
            self,
            DayTok::Ymd { .. } | DayTok::Numeric { .. } | DayTok::Serial(_)
        )
    }
}

pub fn weekday_word(word: &str) -> Option<Weekday> {
    let word = word.trim_end_matches(['.', ',', ':']);
    Some(match word {
        "mon" | "monday" | "mo" | "lun" | "lunes" | "lu" => Weekday::Mon,
        "tue" | "tues" | "tuesday" | "tu" | "martes" | "mar" | "ma" => Weekday::Tue,
        "wed" | "weds" | "wednesday" | "we" | "miercoles" | "mie" | "mi" | "mier" => Weekday::Wed,
        "thu" | "thur" | "thurs" | "thursday" | "jueves" | "jue" | "ju" => Weekday::Thu,
        "fri" | "friday" | "fr" | "viernes" | "vie" | "vi" => Weekday::Fri,
        "sat" | "saturday" | "sabado" | "sab" | "sa" => Weekday::Sat,
        "sun" | "sunday" | "su" | "domingo" | "dom" | "do" => Weekday::Sun,
        _ => return None,
    })
}

/// Whether a person or site name is really a date or day that leaked into a
/// name position. Real dates, Excel serials, single letters and full or
/// standard day words ("Mon", "Monday", "lunes") are. The short and Spanish
/// abbreviations the date reader also accepts ("Mo", "Lu", "Dom", "Mar",
/// "Sal" isn't one) are common first names too, so as a name they stay names
/// (the Phase 7 holdout lost every assignment of a worker called Mo).
pub fn is_day_not_name(text: &str) -> bool {
    let Some(tok) = parse(text) else {
        return false;
    };
    match tok {
        DayTok::Weekday(_) => {
            let k = key(text);
            let k = k.trim_matches(|c: char| c == ':' || c == '*' || c == '.' || c.is_whitespace());
            matches!(
                k,
                "mon"
                    | "monday"
                    | "tue"
                    | "tues"
                    | "tuesday"
                    | "wed"
                    | "weds"
                    | "wednesday"
                    | "thu"
                    | "thur"
                    | "thurs"
                    | "thursday"
                    | "fri"
                    | "friday"
                    | "sat"
                    | "saturday"
                    | "sun"
                    | "sunday"
                    | "lunes"
                    | "martes"
                    | "miercoles"
                    | "jueves"
                    | "viernes"
                    | "sabado"
                    | "domingo"
            )
        }
        _ => true,
    }
}

fn letter(word: &str) -> Option<Letter> {
    Some(match word.trim_end_matches('.') {
        "m" => Letter::M,
        "t" => Letter::T,
        "w" => Letter::W,
        "th" | "r" | "h" => Letter::Th,
        "f" => Letter::F,
        "s" => Letter::S,
        "sa" => Letter::Sa,
        "su" | "u" => Letter::Su,
        _ => return None,
    })
}

fn letter_fits(l: Letter, w: Weekday) -> bool {
    match l {
        Letter::M => w == Weekday::Mon,
        Letter::T => w == Weekday::Tue || w == Weekday::Thu,
        Letter::W => w == Weekday::Wed,
        Letter::Th => w == Weekday::Thu,
        Letter::F => w == Weekday::Fri,
        Letter::S => w == Weekday::Sat || w == Weekday::Sun,
        Letter::Sa => w == Weekday::Sat,
        Letter::Su => w == Weekday::Sun,
    }
}

fn year_of(text: &str) -> Option<i32> {
    let y: i32 = text.parse().ok()?;
    Some(match text.len() {
        2 => 2000 + y,
        4 if (1990..=2100).contains(&y) => y,
        _ => return None,
    })
}

static ISO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{4})-(\d{1,2})-(\d{1,2})(?:[t ].*)?$").expect("regex"));
static NUMERIC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{1,2})\s*([/.\-])\s*(\d{1,2})(?:\s*[/.\-]\s*(\d{4}|\d{2}))?$").expect("regex")
});
static MONTH_DAY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([a-z]{3,10})\.?\s*(\d{1,2})(?:st|nd|rd|th)?(?:\s*,?\s*(\d{4}))?$")
        .expect("regex")
});
static DAY_MONTH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{1,2})(?:st|nd|rd|th)?(?:\s*[\- ]\s*|\s+de\s+)([a-z]{3,10})\.?(?:\s*(?:[\- ,]|\s+de\s+)\s*(\d{4}|\d{2}))?$")
        .expect("regex")
});

/// Parses one cell's text as a day or date. Never guesses a time range
/// ("7-3") as a date: hyphenated numbers count only when the second part is a
/// plausible day and the caller is reading a date context.
pub fn parse(text: &str) -> Option<DayTok> {
    let k = key(text);
    let k = k.trim_matches(|c: char| c == ':' || c == '*' || c.is_whitespace());
    if k.is_empty() || k.len() > 40 {
        return None;
    }
    if let Some(w) = weekday_word(k) {
        return Some(DayTok::Weekday(w));
    }
    if let Some(l) = letter(k) {
        return Some(DayTok::Letter(l));
    }
    if let Some(tok) = parse_date_part(k) {
        return Some(tok);
    }
    // Weekday before or after the date: "mon 10/12", "monday, oct 12", "10/12 mon", "lunes 12 de octubre".
    let words: Vec<&str> = k.split([' ', ',', '-']).filter(|w| !w.is_empty()).collect();
    if words.len() >= 2 {
        if let Some(w) = weekday_word(words[0]) {
            let rest = k[k.find(words[0]).unwrap_or(0) + words[0].len()..].trim_start_matches(
                |c: char| c == ',' || c == '-' || c == '.' || c.is_whitespace(),
            );
            if let Some(tok) = parse_date_part(rest) {
                return Some(with_weekday(tok, w));
            }
        }
        if let Some(w) = weekday_word(words[words.len() - 1]) {
            let last = words[words.len() - 1];
            let rest = k[..k.rfind(last).unwrap_or(k.len())]
                .trim_end_matches(|c: char| c == ',' || c == '-' || c.is_whitespace());
            if let Some(tok) = parse_date_part(rest) {
                return Some(with_weekday(tok, w));
            }
        }
    }
    None
}

fn with_weekday(tok: DayTok, w: Weekday) -> DayTok {
    match tok {
        DayTok::Ymd {
            year, month, day, ..
        } => DayTok::Ymd {
            year,
            month,
            day,
            weekday: Some(w),
        },
        DayTok::Numeric {
            first,
            second,
            year,
            ..
        } => DayTok::Numeric {
            first,
            second,
            year,
            weekday: Some(w),
        },
        other => other,
    }
}

fn parse_date_part(k: &str) -> Option<DayTok> {
    if let Some(c) = ISO.captures(k) {
        let (y, m, d) = (c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?);
        NaiveDate::from_ymd_opt(y, m, d)?;
        return Some(DayTok::Ymd {
            year: Some(y),
            month: m,
            day: d,
            weekday: None,
        });
    }
    if let Some(c) = NUMERIC.captures(k) {
        let first: u32 = c[1].parse().ok()?;
        let second: u32 = c[3].parse().ok()?;
        let year = match c.get(4) {
            Some(y) => Some(year_of(y.as_str())?),
            None => None,
        };
        // "7-3" without a year is a time range, not a date; require "/" or
        // "." or a year for hyphenated forms unless both parts look like a
        // month and day of a plausible date.
        if &c[2] == "-"
            && year.is_none()
            && !(first <= 12 && second <= 31 && second > 0 && (first >= 10 || second >= 13))
        {
            return None;
        }
        if first == 0 || second == 0 || first > 31 || second > 31 || (first > 12 && second > 12) {
            return None;
        }
        return Some(DayTok::Numeric {
            first,
            second,
            year,
            weekday: None,
        });
    }
    if let Some(c) = MONTH_DAY.captures(k) {
        let month = is_month_word(&c[1])?;
        let day: u32 = c[2].parse().ok()?;
        let year = c.get(3).and_then(|y| year_of(y.as_str()));
        if !(1..=31).contains(&day) {
            return None;
        }
        return Some(DayTok::Ymd {
            year,
            month,
            day,
            weekday: None,
        });
    }
    if let Some(c) = DAY_MONTH.captures(k) {
        let month = is_month_word(&c[2])?;
        let day: u32 = c[1].parse().ok()?;
        let year = c.get(3).and_then(|y| year_of(y.as_str()));
        if !(1..=31).contains(&day) {
            return None;
        }
        return Some(DayTok::Ymd {
            year,
            month,
            day,
            weekday: None,
        });
    }
    None
}

/// A whole-number Excel serial in the plausible schedule range (1982–2119).
pub fn serial(value: f64) -> Option<i64> {
    (value.fract() == 0.0 && (30_000.0..80_000.0).contains(&value)).then_some(value as i64)
}

/// The year that puts month/day closest to `today`.
pub fn closest_year(month: u32, day: u32, today: NaiveDate) -> Option<NaiveDate> {
    [today.year() - 1, today.year(), today.year() + 1]
        .into_iter()
        .filter_map(|y| NaiveDate::from_ymd_opt(y, month, day))
        .min_by_key(|d| (*d - today).num_days().abs())
}

/// Whether a set of numeric dates is written day-first: only if some value
/// has a first part over 12 (and none has a second part over 12).
pub fn day_first(toks: &[DayTok]) -> bool {
    let mut first_big = false;
    let mut second_big = false;
    for t in toks {
        if let DayTok::Numeric { first, second, .. } = t {
            first_big |= *first > 12;
            second_big |= *second > 12;
        }
    }
    first_big && !second_big
}

/// Resolves one token to a date given the column's order and today. Weekday
/// and letter tokens need an anchor and are not resolved here.
pub fn resolve(tok: &DayTok, day_first: bool, today: NaiveDate) -> Option<NaiveDate> {
    let (month, day, year) = match *tok {
        DayTok::Ymd {
            year, month, day, ..
        } => (month, day, year),
        DayTok::Numeric {
            first,
            second,
            year,
            ..
        } => {
            if day_first {
                (second, first, year)
            } else {
                (first, second, year)
            }
        }
        DayTok::Serial(s) => return crate::read::serial_to_date(s as f64),
        DayTok::Weekday(_) | DayTok::Letter(_) => return None,
    };
    match year {
        Some(y) => NaiveDate::from_ymd_opt(y, month, day),
        None => closest_year(month, day, today),
    }
}

/// Resolves a run of tokens that should be distinct days in order (a header
/// row or column). Dates are resolved directly; weekday names and letters
/// are read as a sequence and placed in the week of `anchor`.
///
/// Returns one date per token, or `None` for the whole run when it isn't a
/// consistent sequence.
pub fn resolve_run(
    toks: &[DayTok],
    anchor: Option<NaiveDate>,
    today: NaiveDate,
    day_first_hint: bool,
) -> Option<Vec<NaiveDate>> {
    if toks.is_empty() {
        return None;
    }
    if toks.iter().all(DayTok::has_date) {
        let resolve_all = |df: bool| -> Option<Vec<NaiveDate>> {
            let mut dates: Vec<NaiveDate> = toks
                .iter()
                .map(|t| resolve(t, df, today))
                .collect::<Option<_>>()?;
            fix_year_rollover(toks, &mut dates);
            Some(dates)
        };
        let df = day_first(toks) || (day_first_hint && !second_over_12(toks));
        let dates = resolve_all(df)?;
        // An all-ambiguous header stays month-first, unless that spreads its
        // days over months while day-first makes them a week.
        if !df
            && !second_over_12(toks)
            && span_days(&dates) > 31
            && let Some(alt) = resolve_all(true).filter(|alt| span_days(alt) <= 14)
        {
            return Some(alt);
        }
        return Some(dates);
    }
    if toks.iter().any(DayTok::has_date) {
        return None;
    }
    let weekdays = weekday_sequence(toks)?;
    let anchor = anchor?;
    Some(weekdays_in_week(&weekdays, anchor))
}

fn second_over_12(toks: &[DayTok]) -> bool {
    toks.iter()
        .any(|t| matches!(t, DayTok::Numeric { second, .. } if *second > 12))
}

fn span_days(dates: &[NaiveDate]) -> i64 {
    match (dates.iter().min(), dates.iter().max()) {
        (Some(a), Some(b)) => (*b - *a).num_days(),
        _ => 0,
    }
}

/// Without a written year, "12/28 … 1/3" must roll into the next year.
fn fix_year_rollover(toks: &[DayTok], dates: &mut [NaiveDate]) {
    let yearless = |t: &DayTok| {
        matches!(
            t,
            DayTok::Ymd { year: None, .. } | DayTok::Numeric { year: None, .. }
        )
    };
    for i in 1..dates.len() {
        if yearless(&toks[i])
            && dates[i] < dates[i - 1]
            && (dates[i - 1] - dates[i]).num_days() > 300
            && let Some(next) =
                NaiveDate::from_ymd_opt(dates[i].year() + 1, dates[i].month(), dates[i].day())
        {
            dates[i] = next;
        }
        if yearless(&toks[i - 1])
            && dates[i] > dates[i - 1]
            && (dates[i] - dates[i - 1]).num_days() > 300
            && let Some(next) = NaiveDate::from_ymd_opt(
                dates[i - 1].year() + 1,
                dates[i - 1].month(),
                dates[i - 1].day(),
            )
        {
            dates[i - 1] = next;
        }
    }
}

/// Reads weekday names and single letters as consecutive days.
pub fn weekday_sequence(toks: &[DayTok]) -> Option<Vec<Weekday>> {
    let starts: Vec<Weekday> = match toks.first()? {
        DayTok::Weekday(w) => vec![*w],
        DayTok::Letter(l) => [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ]
        .into_iter()
        .filter(|w| letter_fits(*l, *w))
        .collect(),
        _ => return None,
    };
    // Prefer a sequence starting Monday, then Sunday, then any.
    let mut order = starts.clone();
    order.sort_by_key(|w| match w {
        Weekday::Mon => 0,
        Weekday::Sun => 1,
        _ => 2,
    });
    'start: for start in order {
        let mut out = Vec::with_capacity(toks.len());
        let mut day = start;
        for (i, t) in toks.iter().enumerate() {
            if i > 0 {
                day = day.succ();
            }
            let ok = match t {
                DayTok::Weekday(w) => *w == day,
                DayTok::Letter(l) => letter_fits(*l, day),
                _ => false,
            };
            if !ok {
                // Named weekdays may skip days (Mon, Tue, Thu): follow the names.
                if let DayTok::Weekday(w) = t
                    && i > 0
                    && out.last().is_some_and(|prev: &Weekday| {
                        prev.num_days_from_monday() < w.num_days_from_monday()
                    })
                {
                    day = *w;
                    out.push(day);
                    continue;
                }
                continue 'start;
            }
            out.push(day);
        }
        return Some(out);
    }
    None
}

/// The date of weekday `w` in the Monday–Sunday week containing `date`.
pub fn weekday_in_week(date: NaiveDate, w: Weekday) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
        + Duration::days(w.num_days_from_monday() as i64)
}

/// Places weekdays in the week that starts on `anchor` ("Week of 10/12").
pub fn weekdays_in_week(weekdays: &[Weekday], anchor: NaiveDate) -> Vec<NaiveDate> {
    weekdays
        .iter()
        .map(|w| {
            let offset = (w.num_days_from_monday() as i64
                - anchor.weekday().num_days_from_monday() as i64)
                .rem_euclid(7);
            anchor + Duration::days(offset)
        })
        .collect()
}

static WEEK_OF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:week\s*(?:of|starting|beginning|begins|start(?:ing)?)?|wk\s*(?:of)?|w/?e|semana\s*(?:del?)?|for the week of|starting|beginning)\s*[:\-]?\s*(.+)$")
        .expect("regex")
});

/// Finds a "Week of 10/12/2026" (or "Semana del 12 de octubre") anchor.
pub fn week_anchor(text: &str, today: NaiveDate) -> Option<NaiveDate> {
    let k = key(text);
    let c = WEEK_OF.captures(&k)?;
    let rest = c[1].trim();
    // "10/12/2026 - 10/16/2026": take the first date.
    for candidate in [
        rest,
        rest.split([' ', '–', '—']).next().unwrap_or(rest),
        rest.split(" - ").next().unwrap_or(rest),
        rest.split(" to ").next().unwrap_or(rest),
    ] {
        if let Some(tok) = parse(candidate.trim_end_matches(['.', ',', ':']))
            && tok.has_date()
        {
            let df = day_first(&[tok]);
            return resolve(&tok, df, today);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }
    const TODAY: fn() -> NaiveDate = || d(2026, 10, 10);

    #[test]
    fn formats() {
        let t = TODAY();
        let one = |s: &str| {
            let tok = parse(s).unwrap_or_else(|| panic!("no parse: {s}"));
            resolve(&tok, day_first(&[tok]), t)
        };
        assert_eq!(one("2026-10-12"), Some(d(2026, 10, 12)));
        assert_eq!(one("10/12/2026"), Some(d(2026, 10, 12)));
        assert_eq!(one("10/12/26"), Some(d(2026, 10, 12)));
        assert_eq!(one("10/12"), Some(d(2026, 10, 12)));
        assert_eq!(one("Oct 12"), Some(d(2026, 10, 12)));
        assert_eq!(one("October 12, 2026"), Some(d(2026, 10, 12)));
        assert_eq!(one("12-Oct"), Some(d(2026, 10, 12)));
        assert_eq!(one("12 de octubre"), Some(d(2026, 10, 12)));
        assert_eq!(one("Mon 10/12"), Some(d(2026, 10, 12)));
        assert_eq!(one("Monday, Oct 12"), Some(d(2026, 10, 12)));
        assert_eq!(one("13/10/2026"), Some(d(2026, 10, 13)));
        assert_eq!(
            parse("Mon 10/13/2026").unwrap().written_weekday(),
            Some(Weekday::Mon)
        );
        assert!(parse("7-3").is_none());
        assert!(parse("7-11").is_none());
        assert!(parse("Mike").is_none());
    }

    #[test]
    fn runs() {
        let t = TODAY();
        let toks: Vec<DayTok> = ["M", "T", "W", "Th", "F"]
            .iter()
            .map(|s| parse(s).unwrap())
            .collect();
        let dates = resolve_run(&toks, Some(d(2026, 10, 12)), t, false).unwrap();
        assert_eq!(dates[3], d(2026, 10, 15));
        let toks: Vec<DayTok> = ["M", "T", "W", "T", "F", "S", "S"]
            .iter()
            .map(|s| parse(s).unwrap())
            .collect();
        let dates = resolve_run(&toks, Some(d(2026, 10, 12)), t, false).unwrap();
        assert_eq!(dates[6], d(2026, 10, 18));
        let toks: Vec<DayTok> = ["12/28", "12/29", "12/30", "12/31", "1/1", "1/2", "1/3"]
            .iter()
            .map(|s| parse(s).unwrap())
            .collect();
        let dates = resolve_run(&toks, None, t, false).unwrap();
        assert_eq!(dates[0], d(2026, 12, 28));
        assert_eq!(dates[6], d(2027, 1, 3));
        let toks: Vec<DayTok> = ["5/10", "6/10", "7/10", "13/10"]
            .iter()
            .map(|s| parse(s).unwrap())
            .collect();
        let dates = resolve_run(&toks, None, t, false).unwrap();
        assert_eq!(dates[0], d(2026, 10, 5));
    }

    #[test]
    fn anchors() {
        assert_eq!(
            week_anchor("Week of 10/12/2026", TODAY()),
            Some(d(2026, 10, 12))
        );
        assert_eq!(
            week_anchor("WEEK OF: Oct 12", TODAY()),
            Some(d(2026, 10, 12))
        );
        assert_eq!(
            week_anchor("Semana del 12 de octubre", TODAY()),
            Some(d(2026, 10, 12))
        );
        assert_eq!(week_anchor("Crew schedule", TODAY()), None);
    }
}
