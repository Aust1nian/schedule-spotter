//! Time ranges inside cells: "7-11", "7a-3p", "7:00–15:30", "6-2:30",
//! "10pm-6am", "0700-1530". Hours without am/pm follow a construction day:
//! a start of 5–11 is morning, 12 is noon, 1–4 is afternoon, and the end is
//! the first matching hour after the start.

use std::sync::LazyLock;

use regex::Regex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeRange {
    pub start: u16,
    pub end: u16,
}

static RANGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        (?P<h1>\d{1,2})(?::(?P<m1>\d{2}))?\s*(?P<ap1>a\.?m\.?|p\.?m\.?|a|p)?
        \s*(?:-|–|—|to|till|til|until|a|hasta|thru)\s*
        (?P<h2>\d{1,2})(?::(?P<m2>\d{2}))?\s*(?P<ap2>a\.?m\.?|p\.?m\.?|a|p)?",
    )
    .expect("regex")
});

static MILITARY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?P<h1>[0-2]\d)(?P<m1>[0-5]\d)\s*(?:-|–|—|to)\s*(?P<h2>[0-2]\d)(?P<m2>[0-5]\d)")
        .expect("regex")
});

static SINGLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?ix)^(?P<h>\d{1,2})(?::(?P<m>\d{2}))?\s*(?P<ap>a\.?m\.?|p\.?m\.?|a|p)?$")
        .expect("regex")
});

fn is_pm(ap: Option<&str>) -> Option<bool> {
    let ap = ap?.to_ascii_lowercase();
    Some(ap.starts_with('p'))
}

fn clock(h: u32, m: u32, pm: Option<bool>) -> Option<u16> {
    if m > 59 || h > 24 {
        return None;
    }
    let h = match pm {
        Some(true) if h < 12 => h + 12,
        Some(false) if h == 12 => 0,
        Some(_) if h > 12 => return None,
        _ => h,
    };
    Some(((h % 24) * 60 + m) as u16)
}

/// Resolves "h1[:m1][ap1] - h2[:m2][ap2]" into minutes.
fn resolve(
    h1: u32,
    m1: u32,
    ap1: Option<bool>,
    h2: u32,
    m2: u32,
    ap2: Option<bool>,
) -> Option<TimeRange> {
    if h1 > 24 || h2 > 24 || m1 > 59 || m2 > 59 {
        return None;
    }
    let start = match (ap1, ap2) {
        (Some(p), _) => clock(h1, m1, Some(p))?,
        // "7-3pm" → 7am; "1-5pm" → 1pm; "11-1pm" → 11am.
        (None, Some(p2)) if h1 <= 12 => {
            if p2 && h1 != 12 && (h1 > h2 || h2 == 12) {
                clock(h1, m1, Some(false))?
            } else {
                clock(h1, m1, Some(p2))?
            }
        }
        _ if h1 >= 13 || h1 == 0 => clock(h1, m1, None)?,
        _ if (1..=4).contains(&h1) => clock(h1 + 12, m1, None)?,
        _ => clock(h1, m1, None)?, // 5..=12 as written
    };
    let end = match ap2 {
        Some(p) => clock(h2, m2, Some(p))?,
        None if h2 >= 13 || h2 == 0 => clock(h2, m2, None)?,
        None => {
            // The first of h2 and h2+12 that comes after the start.
            let am = clock(h2, m2, None)?;
            let pm = clock(if h2 == 12 { 12 } else { h2 + 12 }, m2, None)?;
            if am > start {
                am
            } else if pm > start {
                pm
            } else {
                am // past midnight
            }
        }
    };
    (start != end).then_some(TimeRange { start, end })
}

/// Finds the first time range in `text`. Returns the range and the byte span
/// it occupied so the caller can remove it.
pub fn find_range(text: &str) -> Option<(TimeRange, std::ops::Range<usize>)> {
    let bytes = text.as_bytes();
    let bounded = |span: &std::ops::Range<usize>| {
        let before = span.start.checked_sub(1).map(|i| bytes[i]);
        let after = bytes.get(span.end).copied();
        !before.is_some_and(|b| {
            b.is_ascii_alphanumeric() || b == b'/' || b == b'#' || b == b'-' || b == b'.'
        }) && !after.is_some_and(|b| b.is_ascii_digit() || b == b'/' || b.is_ascii_alphabetic())
    };
    for c in RANGE.captures_iter(text) {
        let whole = c.get(0)?;
        let mut span = whole.range();
        // The regex may swallow a trailing space; trim it from the span.
        while span.end > span.start && bytes[span.end - 1] == b' ' {
            span.end -= 1;
        }
        if !bounded(&span) {
            continue;
        }
        let num = |name: &str| c.name(name).map(|m| m.as_str().parse::<u32>().unwrap_or(0));
        let ap = |name: &str| is_pm(c.name(name).map(|m| m.as_str()));
        // A bare "a" separator between numbers ("7 a 3") is fine; but "a"/"p"
        // right after a number must not swallow a word ("7 apples").
        if let Some(range) = resolve(
            num("h1")?,
            num("m1").unwrap_or(0),
            ap("ap1"),
            num("h2")?,
            num("m2").unwrap_or(0),
            ap("ap2"),
        ) {
            return Some((range, span));
        }
    }
    for c in MILITARY.captures_iter(text) {
        let span = c.get(0)?.range();
        if !bounded(&span) {
            continue;
        }
        let num = |name: &str| c.name(name).map(|m| m.as_str().parse::<u32>().unwrap_or(0));
        let start = clock(num("h1")?, num("m1")?, None)?;
        let end = clock(num("h2")?, num("m2")?, None)?;
        if start != end {
            return Some((TimeRange { start, end }, span));
        }
    }
    None
}

/// A whole cell that is just a time range.
pub fn parse_range(text: &str) -> Option<TimeRange> {
    let t = text.trim();
    let (range, span) = find_range(t)?;
    let rest = format!("{}{}", &t[..span.start], &t[span.end..]);
    rest.trim().is_empty().then_some(range)
}

/// A single clock time: "7", "7:30", "7am", "15:30".
pub fn parse_single(text: &str) -> Option<u16> {
    let c = SINGLE.captures(text.trim())?;
    let h: u32 = c.name("h")?.as_str().parse().ok()?;
    let m: u32 = c
        .name("m")
        .map(|m| m.as_str().parse().unwrap_or(0))
        .unwrap_or(0);
    let pm = is_pm(c.name("ap").map(|m| m.as_str()));
    if pm.is_none() && c.name("m").is_none() && h > 12 && h <= 24 {
        return clock(h, 0, None);
    }
    match pm {
        Some(p) => clock(h, m, Some(p)),
        None if (1..=4).contains(&h) => clock(h + 12, m, None),
        None => clock(h, m, None),
    }
}

/// Builds a range from separate start and end cells.
pub fn from_pair(start: &str, end: &str) -> Option<TimeRange> {
    let s = SINGLE.captures(start.trim())?;
    let e = SINGLE.captures(end.trim())?;
    let num =
        |c: &regex::Captures, n: &str| c.name(n).map(|m| m.as_str().parse::<u32>().unwrap_or(0));
    resolve(
        num(&s, "h")?,
        num(&s, "m").unwrap_or(0),
        is_pm(s.name("ap").map(|m| m.as_str())),
        num(&e, "h")?,
        num(&e, "m").unwrap_or(0),
        is_pm(e.name("ap").map(|m| m.as_str())),
    )
}

pub fn fmt(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> Option<(String, String)> {
        parse_range(s).map(|t| (fmt(t.start), fmt(t.end)))
    }

    #[test]
    fn ranges() {
        let pair = |a: &str, b: &str| Some((a.to_string(), b.to_string()));
        assert_eq!(r("7-11"), pair("07:00", "11:00"));
        assert_eq!(r("7-3"), pair("07:00", "15:00"));
        assert_eq!(r("7a-3p"), pair("07:00", "15:00"));
        assert_eq!(r("7:00–15:30"), pair("07:00", "15:30"));
        assert_eq!(r("6-2:30"), pair("06:00", "14:30"));
        assert_eq!(r("1-5"), pair("13:00", "17:00"));
        assert_eq!(r("7-3pm"), pair("07:00", "15:00"));
        assert_eq!(r("10pm-6am"), pair("22:00", "06:00"));
        assert_eq!(r("10p-6a"), pair("22:00", "06:00"));
        assert_eq!(r("0700-1530"), pair("07:00", "15:30"));
        assert_eq!(r("7 to 3:30"), pair("07:00", "15:30"));
        assert_eq!(r("12-4"), pair("12:00", "16:00"));
        assert_eq!(r("8am - 12pm"), pair("08:00", "12:00"));
        assert_eq!(r("8-12"), pair("08:00", "12:00"));
    }

    #[test]
    fn not_ranges() {
        assert!(find_range("1420 Johnson St").is_none());
        assert!(find_range("J-1042 Oak Ridge").is_none());
        assert!(find_range("10/12-10/13").is_none());
        assert!(find_range("Building 2-B").is_none());
    }

    #[test]
    fn inside_text() {
        let (range, span) = find_range("Johnson 7-3:30").unwrap();
        assert_eq!(
            (fmt(range.start), fmt(range.end)),
            ("07:00".into(), "15:30".into())
        );
        assert_eq!(&"Johnson 7-3:30"[span], "7-3:30");
        assert_eq!(parse_single("7am"), Some(420));
        assert_eq!(parse_single("15:30"), Some(930));
        assert_eq!(from_pair("7:00", "3:30").map(|t| t.end), Some(930));
    }
}
