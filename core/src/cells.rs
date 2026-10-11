//! Stage 6: one cell's text → names or sites, with statuses, unstaffed and
//! inactive markers, times, parts of the day and notes split off.
//!
//! A trailing note (after " - " or in a final "(…)") belongs to the whole
//! cell and is copied onto every record the cell produces; a "(…)" right
//! after one name belongs to that name only.

use crate::classify::site_score;
use crate::lexicon::{JOB_WORDS, is_inactive_marker, is_unstaffed_marker, part_word, status_word};
use crate::model::{Part, Status};
use crate::text::{clean, key};
use crate::times::{self, TimeRange};

/// What a cell is expected to hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    /// Names: the body of a job grid, or a person column.
    People,
    /// Job sites: the body of a person grid, or a site column.
    Sites,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    pub text: String,
    pub note: Option<String>,
    pub part: Option<Part>,
    pub time: Option<TimeRange>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parsed {
    pub items: Vec<Item>,
    pub status: Option<(Status, Option<Part>)>,
    pub unstaffed: bool,
    /// x, —, n/a, "no work", "done": in a job grid, the job isn't running.
    pub inactive: bool,
    pub time: Option<TimeRange>,
    pub part: Option<Part>,
    pub note: Option<String>,
}

impl Parsed {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.status.is_none() && !self.unstaffed && !self.inactive
    }
}

pub fn parse_cell(text: &str, expect: Expect) -> Parsed {
    let mut out = Parsed::default();
    let raw = text.replace('\r', "");
    let whole = key(&raw);
    if whole.is_empty() {
        return out;
    }
    if is_inactive_marker(&whole) {
        out.inactive = true;
    }
    for line in raw.split('\n') {
        parse_line(line, expect, &mut out);
    }
    // A cell that is only an inactive marker carries no other meaning in a
    // job grid; in a person grid the same marks ("x", "—") mean off.
    if out.inactive
        && out.items.is_empty()
        && out.status.is_none()
        && let Some(s) = status_word(&whole)
    {
        out.status = Some((s, None));
    }
    out
}

fn parse_line(line: &str, expect: Expect, out: &mut Parsed) {
    let mut line = clean(&normalize_halves(line));
    if line.is_empty() {
        return;
    }
    // Whole-line status, unstaffed, part.
    if let Some(found) = status_with_part(&line) {
        out.status = Some(found);
        return;
    }
    if is_unstaffed_marker(&key(&line)) {
        out.unstaffed = true;
        return;
    }
    if is_inactive_marker(&key(&line)) {
        out.inactive = true;
        return;
    }

    // One time anywhere in the line applies to the whole line; with several
    // ("Smith 7-11 / Parker 12-4") each segment keeps its own.
    let mut line_time = None;
    let several_times = count_ranges(&line) > 1;
    if let Some((range, span)) = times::find_range(&line).filter(|_| !several_times) {
        line_time = Some(range);
        line = clean(&format!("{} {}", &line[..span.start], &line[span.end..]));
        line = line
            .trim_matches(|c: char| c == ',' || c == '-' || c == '@' || c.is_whitespace())
            .to_string();
    }

    // Trailing cell-level note: final "(…)" or text after the last " - ".
    let mut line_note = None;
    if let Some((before, inside)) = trailing_paren(&line) {
        let inside_key = key(&inside);
        if let Some(p) = part_word(&inside_key) {
            out.part = out.part.or(Some(p));
            line = before;
        } else if is_segment_list(&before, expect) || expect == Expect::Sites {
            line_note = Some(inside);
            line = before;
        }
    }
    if let Some((before, after)) = trailing_dash_note(&line) {
        let after_key = key(&after);
        if let Some(p) = part_word(&after_key) {
            out.part = out.part.or(Some(p));
            line = before;
        } else if let Some(found) = status_word(&after_key) {
            // "Luis - off": a status note on a name in a job grid.
            line = before;
            out.status = out.status.or(Some((found, None)));
        } else {
            line_note = Some(after);
            line = before;
        }
    }

    let segments = split_segments(&line, expect);
    let mut produced = 0;
    for seg in segments {
        let mut seg = seg.trim().to_string();
        if seg.is_empty() {
            continue;
        }
        if let Some(found) = status_with_part(&seg) {
            out.status = Some(found);
            continue;
        }
        let k = key(&seg);
        if is_unstaffed_marker(&k) {
            out.unstaffed = true;
            continue;
        }
        if let Some(p) = part_word(&k) {
            out.part = out.part.or(Some(p));
            continue;
        }
        let mut item = Item::default();
        if several_times && let Some((range, span)) = times::find_range(&seg) {
            item.time = Some(range);
            seg = clean(&format!("{} {}", &seg[..span.start], &seg[span.end..]));
        }
        if let Some((before, inside)) = trailing_paren(&seg) {
            match part_word(&key(&inside)) {
                Some(p) => item.part = Some(p),
                None => item.note = Some(inside),
            }
            seg = before;
        }
        let (text, part) = strip_part(&seg);
        item.part = item.part.or(part);
        item.text = text;
        if item.text.is_empty() {
            if item.part.is_some() {
                out.part = out.part.or(item.part);
            }
            continue;
        }
        if let Some(n) = &line_note {
            item.note = Some(match item.note.take() {
                Some(own) => format!("{own}; {n}"),
                None => n.clone(),
            });
        }
        out.items.push(item);
        produced += 1;
    }
    if let Some(t) = line_time {
        if produced > 0 {
            let n = out.items.len();
            for item in &mut out.items[n - produced..] {
                item.time = item.time.or(Some(t));
            }
        } else {
            out.time = out.time.or(Some(t));
        }
    }
    if produced == 0
        && let Some(n) = line_note
    {
        out.note = Some(n);
    }
}

fn count_ranges(line: &str) -> usize {
    let mut n = 0;
    let mut rest = line;
    while let Some((_, span)) = times::find_range(rest) {
        n += 1;
        rest = &rest[span.end..];
    }
    n
}

/// "1/2 day" and "½" read as "half day" before any "/" splitting.
fn normalize_halves(line: &str) -> String {
    line.replace("1/2 day", "half day")
        .replace("1/2 Day", "half day")
        .replace("½ day", "half day")
        .replace('½', "half day")
}

/// "off", "OFF PM", "PTO (AM)", "half day off".
fn status_with_part(text: &str) -> Option<(Status, Option<Part>)> {
    let k = key(text);
    if let Some(s) = status_word(&k) {
        return Some((s, None));
    }
    let (rest, part) = strip_part(text);
    if part.is_some() {
        let rk = key(&rest);
        let rk = rk.trim_matches(|c: char| c == '(' || c == ')' || c == '-' || c.is_whitespace());
        if let Some(s) = status_word(rk) {
            return Some((s, part));
        }
    }
    None
}

/// Removes a part-of-day word at the start or end: "Johnson AM" → ("Johnson", Am).
fn strip_part(text: &str) -> (String, Option<Part>) {
    let t = clean(text);
    let lower = key(&t);
    for word in [
        "am only",
        "pm only",
        "half day",
        "half-day",
        "am",
        "pm",
        "a.m.",
        "p.m.",
        "morning",
        "afternoon",
        "manana",
        "tarde",
        "medio dia",
    ] {
        if let Some(p) = part_word(word) {
            if lower == word {
                return (String::new(), Some(p));
            }
            if lower.ends_with(&format!(" {word}")) {
                let cut = t.len() - word.len();
                if t.is_char_boundary(cut) {
                    return (
                        t[..cut].trim_end_matches([' ', '-', ',']).to_string(),
                        Some(p),
                    );
                }
            }
            if lower.starts_with(&format!("{word} ")) && t.is_char_boundary(word.len()) {
                return (
                    t[word.len()..]
                        .trim_start_matches([' ', '-', ':'])
                        .to_string(),
                    Some(p),
                );
            }
        }
    }
    (t, None)
}

fn trailing_paren(text: &str) -> Option<(String, String)> {
    let t = text.trim_end();
    if !t.ends_with(')') {
        return None;
    }
    let open = t.rfind('(')?;
    let inside = t[open + 1..t.len() - 1].trim().to_string();
    let before = t[..open].trim_end().to_string();
    if before.is_empty() || inside.is_empty() {
        return None;
    }
    Some((before, inside))
}

fn trailing_dash_note(text: &str) -> Option<(String, String)> {
    for sep in [" - ", " – ", " — ", " -- "] {
        if let Some(idx) = text.rfind(sep) {
            let before = text[..idx].trim().to_string();
            let after = text[idx + sep.len()..].trim().to_string();
            if !before.is_empty() && !after.is_empty() {
                return Some((before, after));
            }
        }
    }
    None
}

/// Whether `text` holds more than one segment for `expect`.
fn is_segment_list(text: &str, expect: Expect) -> bool {
    split_segments(text, expect).len() > 1
}

pub fn split_segments(text: &str, expect: Expect) -> Vec<String> {
    match expect {
        Expect::People => split_people(text),
        Expect::Sites => split_sites(text),
    }
}

fn split_people(text: &str) -> Vec<String> {
    // "Baker, Mike" (surname, first name) stays one person.
    if let Some((a, b)) = text.split_once(',') {
        let (ka, kb) = (key(a), key(b));
        if !ka.contains(' ')
            && !kb.contains(' ')
            && !b.contains([',', '&', '/', '+'])
            && crate::lexicon::is_surname(&ka)
            && !crate::lexicon::is_first_name(&ka)
            && crate::lexicon::is_first_name(&kb)
        {
            return vec![text.trim().to_string()];
        }
    }
    let mut parts = vec![text.to_string()];
    for sep in [
        ",", ";", "&", "/", "+", " and ", " And ", " AND ", " y ", " Y ", " w/ ", " with ",
    ] {
        parts = parts
            .into_iter()
            .flat_map(|p| p.split(sep).map(str::to_string).collect::<Vec<_>>())
            .collect();
    }
    parts
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn split_sites(text: &str) -> Vec<String> {
    let mut parts = vec![text.to_string()];
    for sep in ["/", ";", " + "] {
        parts = parts
            .into_iter()
            .flat_map(|p| p.split(sep).map(str::to_string).collect::<Vec<_>>())
            .collect();
    }
    // ",", " & " and " and " split only when every side reads as a site
    // (or a status) on its own: never "1420 Johnson St, Fresno", and never
    // "Smith & Jones remodel".
    let mut out = Vec::new();
    for part in parts {
        let mut pieces = vec![part];
        for (sep, min) in [
            (",", 0.45),
            (" & ", 0.45),
            (" and ", 0.45),
            (" And ", 0.45),
            (" y ", 0.45),
        ] {
            pieces = pieces
                .into_iter()
                .flat_map(|p| {
                    let split: Vec<String> = p.split(sep).map(|s| s.trim().to_string()).collect();
                    let standalone = |s: &String| {
                        let k = key(s);
                        status_word(&k).is_some() || is_unstaffed_marker(&k) || site_score(s) >= min
                    };
                    // "Smith & Jones remodel" is one job: a lone word joined to
                    // a name that ends in a job word shares that job word.
                    let shared_suffix = sep != ","
                        && split.len() == 2
                        && !split[0].contains(' ')
                        && split[1].contains(' ')
                        && split[1]
                            .rsplit(' ')
                            .next()
                            .is_some_and(|w| JOB_WORDS.contains(key(w).as_str()))
                        && site_score(&split[0]) < 0.85;
                    let all_sites = split.len() > 1
                        && !shared_suffix
                        && split.iter().all(|s| !s.is_empty() && standalone(s))
                        && !(sep == "," && looks_like_city_suffix(&split));
                    if all_sites { split } else { vec![p] }
                })
                .collect();
        }
        out.extend(pieces);
    }
    out.into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// "1420 Johnson St, Fresno" or "…, CA 93721": a street address then a city.
fn looks_like_city_suffix(parts: &[String]) -> bool {
    parts.len() >= 2
        && site_score(&parts[0]) >= 0.85
        && parts[1..].iter().all(|p| {
            let k = key(p);
            !k.chars().any(|c| c.is_ascii_digit())
                || k.split(' ')
                    .all(|w| w.len() == 2 || w.chars().all(|c| c.is_ascii_digit()))
        })
        && parts[1..].iter().all(|p| site_score(p) < 0.85)
        && parts[1..].iter().all(|p| {
            let k = key(p);
            status_word(&k).is_none() && !is_unstaffed_marker(&k)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(p: &Parsed) -> Vec<&str> {
        p.items.iter().map(|i| i.text.as_str()).collect()
    }

    #[test]
    fn people_cells() {
        let p = parse_cell("Mike, Jose & Luis", Expect::People);
        assert_eq!(names(&p), ["Mike", "Jose", "Luis"]);
        let p = parse_cell("Mike B and Chuy y Memo", Expect::People);
        assert_eq!(names(&p), ["Mike B", "Chuy", "Memo"]);
        let p = parse_cell("Mike\nJose", Expect::People);
        assert_eq!(names(&p), ["Mike", "Jose"]);
        let p = parse_cell("Baker, Mike", Expect::People);
        assert_eq!(names(&p), ["Baker, Mike"]);
        let p = parse_cell("Mike, Jose (bring ladder)", Expect::People);
        assert_eq!(names(&p), ["Mike", "Jose"]);
        assert!(
            p.items
                .iter()
                .all(|i| i.note.as_deref() == Some("bring ladder"))
        );
        let p = parse_cell("Mike (lead), Jose", Expect::People);
        assert_eq!(p.items[0].note.as_deref(), Some("lead"));
        assert_eq!(p.items[1].note, None);
        let p = parse_cell("TBD", Expect::People);
        assert!(p.unstaffed && p.items.is_empty());
        let p = parse_cell("need 1", Expect::People);
        assert!(p.unstaffed);
    }

    #[test]
    fn site_cells() {
        let p = parse_cell("Johnson - bring ladder", Expect::Sites);
        assert_eq!(names(&p), ["Johnson"]);
        assert_eq!(p.items[0].note.as_deref(), Some("bring ladder"));
        let p = parse_cell("Smith (punch list)", Expect::Sites);
        assert_eq!(p.items[0].note.as_deref(), Some("punch list"));
        let p = parse_cell("1420 Johnson St 7-3:30", Expect::Sites);
        assert_eq!(names(&p), ["1420 Johnson St"]);
        assert_eq!(p.items[0].time.map(|t| t.end), Some(930));
        let p = parse_cell("Johnson AM / Smith PM", Expect::Sites);
        assert_eq!(names(&p), ["Johnson", "Smith"]);
        assert_eq!(p.items[0].part, Some(Part::Am));
        assert_eq!(p.items[1].part, Some(Part::Pm));
        let p = parse_cell("OFF", Expect::Sites);
        assert_eq!(p.status, Some((Status::Off, None)));
        let p = parse_cell("Off PM", Expect::Sites);
        assert_eq!(p.status, Some((Status::Off, Some(Part::Pm))));
        let p = parse_cell("PTO", Expect::Sites);
        assert_eq!(p.status, Some((Status::Pto, None)));
        let p = parse_cell("1420 Johnson St, Fresno", Expect::Sites);
        assert_eq!(names(&p), ["1420 Johnson St, Fresno"]);
        let p = parse_cell("Johnson\n7-3", Expect::Sites);
        assert_eq!(names(&p), ["Johnson"]);
        assert_eq!(p.time.map(|t| t.start), Some(420));
        let p = parse_cell("x", Expect::Sites);
        assert!(p.inactive);
        assert_eq!(p.status, Some((Status::Off, None)));
        let p = parse_cell("Smith remodel - half day", Expect::Sites);
        assert_eq!(p.part, Some(Part::Half));
    }
}
