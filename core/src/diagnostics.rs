//! "Save diagnostics…": what happened in this check, with every name, job
//! and file name replaced by placeholders (Person 1, Site 2). The owner sees
//! the text before saving it. Built from structured data, then scrubbed of
//! every name and job spelling the schedule contains, as a second guard.

use std::collections::HashMap;

use crate::model::{Answer, Question, Report};
use crate::session::{Session, Source};

#[derive(Default)]
struct Placeholders {
    people: HashMap<String, String>,
    sites: HashMap<String, String>,
    other: HashMap<String, String>,
}

impl Placeholders {
    fn get(map: &mut HashMap<String, String>, prefix: &str, text: &str) -> String {
        let key = crate::text::key(text);
        let n = map.len() + 1;
        map.entry(key)
            .or_insert_with(|| format!("{prefix} {n}"))
            .clone()
    }
    fn person(&mut self, t: &str) -> String {
        Self::get(&mut self.people, "Person", t)
    }
    fn site(&mut self, t: &str) -> String {
        Self::get(&mut self.sites, "Site", t)
    }
    fn text(&mut self, t: &str) -> String {
        if t.trim().is_empty() {
            return String::new();
        }
        Self::get(&mut self.other, "Text", t)
    }
}

fn question(q: &Question, p: &mut Placeholders) -> String {
    match q {
        Question::SamePerson { a, b } => {
            format!("same person? {} / {}", p.person(a), p.person(b))
        }
        Question::SameSite { a, b } => format!("same job? {} / {}", p.site(a), p.site(b)),
        Question::PickSheet { options, .. } => format!("which sheet? ({} options)", options.len()),
        Question::PickLayout { options, .. } => format!("which layout? {}", options.join(" / ")),
        Question::WeekdayMismatch { used, .. } => format!("weekday contradicts date {used}"),
        Question::FillDown { column, .. } => format!("fill down under {}?", p.text(column)),
        Question::ColumnRole { header, .. } => format!("column role of {}?", p.text(header)),
        Question::CellMeaning { text } => format!("cell meaning of {}", p.text(text)),
        Question::PasteAsTable => "paste a table instead".into(),
    }
}

fn answer(a: &Answer) -> String {
    match a {
        Answer::Yes => "yes".into(),
        Answer::No => "no".into(),
        Answer::Pick(o) => format!("picked {o:?}"),
    }
}

/// The diagnostics text. `app` carries the app's own lines (version,
/// platform, model status, timings), which hold no schedule content.
pub fn text(session: &Session, report: &Report, app: &[String]) -> String {
    let mut p = Placeholders::default();
    let mut out: Vec<String> = vec!["Schedule Spotter diagnostics".into(), String::new()];
    out.extend(app.iter().cloned());
    out.push(String::new());

    // Names first, so placeholders follow the schedule's order.
    for row in &report.week.people {
        p.person(&row.name);
    }
    for job in &report.week.jobs {
        p.site(&job.name);
    }

    let source = match &session.source {
        Source::File { name, bytes } => {
            let ext = std::path::Path::new(name)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            format!("file (.{ext}, {} bytes)", bytes.len())
        }
        Source::Paste { text } => format!("pasted text ({} lines)", text.lines().count()),
        Source::Table { text, .. } => format!("table in the app ({} rows)", text.lines().count()),
    };
    out.push(format!("Source: {source}"));
    let w = session.settings.workdays.0;
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let on: Vec<&str> = days
        .iter()
        .zip(w)
        .filter(|(_, on)| *on)
        .map(|(d, _)| *d)
        .collect();
    out.push(format!(
        "Working days: {}; days off for everyone: {}",
        on.join(" "),
        session.settings.days_off.len()
    ));
    if let Some(e) = report.error {
        out.push(format!("Read error: {e:?}"));
    }
    let r = &report.reading;
    out.push(format!(
        "Layout: {:?} ({:?}); sheet {}",
        r.layout,
        r.arrangement,
        p.text(&r.sheet)
    ));
    for c in &r.columns {
        out.push(format!(
            "  column {}: {} -> {}",
            c.index + 1,
            p.text(&c.header),
            c.role
        ));
    }
    out.push(format!(
        "Read {} assignments, {} people, {} jobs, {} dates ({} to {})",
        r.assignments.len(),
        report.week.people.len(),
        report.week.jobs.len(),
        r.window.len(),
        r.window.first().map(|d| d.to_string()).unwrap_or_default(),
        r.window.last().map(|d| d.to_string()).unwrap_or_default(),
    ));
    out.push(String::new());
    out.push(format!("Findings: {}", report.flags.len()));
    for f in &report.flags {
        let who = f.person.as_deref().map(|x| p.person(x)).unwrap_or_default();
        let mut sites: Vec<String> = f.sites.iter().map(|s| p.site(s)).collect();
        if let Some(s) = &f.site {
            sites.push(p.site(s));
        }
        out.push(format!(
            "  {:?} {} {} {} chips={:?}",
            f.kind,
            f.date,
            who,
            sites.join(", "),
            f.merges
        ));
    }
    out.push(format!("Information: {}", report.info.len()));
    for i in &report.info {
        let sites: Vec<String> = i.sites.iter().map(|s| p.site(s)).collect();
        out.push(format!(
            "  {:?} {} {} {}",
            i.kind,
            i.date,
            p.person(&i.person),
            sites.join(", ")
        ));
    }
    out.push(format!("Matches: {}", report.merges.len()));
    for (n, m) in report.merges.iter().enumerate() {
        let (from, into) = match m.kind {
            crate::matching::MergeKind::Person => (p.person(&m.from), p.person(&m.into)),
            crate::matching::MergeKind::Site => (p.site(&m.from), p.site(&m.into)),
        };
        out.push(format!("  [{n}] {from} as {into} ({:?})", m.reason));
    }
    out.push(format!("Open questions: {}", report.questions.len()));
    for q in &report.questions {
        out.push(format!("  {}", question(q, &mut p)));
    }
    out.push(format!("Answers given: {}", session.answers.items.len()));
    for (q, a) in &session.answers.items {
        out.push(format!("  {} -> {}", question(q, &mut p), answer(a)));
    }
    if !r.notes.is_empty() {
        out.push("Reader notes:".into());
        for n in &r.notes {
            out.push(format!("  {n}"));
        }
    }

    // Second guard: scrub every spelling the schedule holds, longest first.
    let mut text = out.join("\n");
    let mut secrets: Vec<(String, String)> = Vec::new();
    let mut add = |s: &str, ph: String| {
        let s = s.trim();
        if s.chars().count() >= 2 && !s.chars().all(|c| c.is_ascii_digit()) {
            secrets.push((s.to_string(), ph));
        }
    };
    for a in &r.assignments {
        if let Some(x) = &a.person {
            add(x, p.person(x));
        }
        if let Some(x) = &a.site {
            add(x, p.site(x));
        }
        if let Some(x) = &a.note {
            add(x, p.text(x));
        }
    }
    for x in r.roster.iter().flatten() {
        add(x, p.person(x));
    }
    for x in &r.grid_jobs {
        add(x, p.site(x));
    }
    for m in &report.merges {
        add(&m.from, p.text(&m.from));
        add(&m.into, p.text(&m.into));
    }
    for c in &r.columns {
        add(&c.header, p.text(&c.header));
    }
    add(&r.sheet, p.text(&r.sheet));
    add(session.name(), "the schedule".into());
    if let Source::File { name, .. } = &session.source
        && let Some(stem) = std::path::Path::new(name).file_stem()
    {
        add(&stem.to_string_lossy(), "the schedule".into());
    }
    secrets.sort_by_key(|(s, _)| std::cmp::Reverse(s.len()));
    for (s, ph) in secrets {
        text = replace_ci(&text, &s, &ph);
    }
    text.push('\n');
    text
}

/// Where `needle` occurs in `text` as whole words, ignoring ASCII case.
fn word_matches(text: &str, needle: &str) -> Vec<usize> {
    let hay = text.to_ascii_lowercase();
    let pat = needle.to_ascii_lowercase();
    if pat.is_empty() || hay.len() != text.len() {
        return Vec::new();
    }
    let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    hay.match_indices(&pat)
        .map(|(i, _)| i)
        .filter(|&i| !word(text[..i].chars().last()) && !word(text[i + pat.len()..].chars().next()))
        .collect()
}

/// Replaces whole-word occurrences, ignoring ASCII case.
fn replace_ci(text: &str, needle: &str, with: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for i in word_matches(text, needle) {
        if i < last {
            continue;
        }
        out.push_str(&text[last..i]);
        out.push_str(with);
        last = i + needle.len();
    }
    out.push_str(&text[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn no_names_leave() {
        let mut s = Session::example(Settings::default());
        s.answer(
            Question::SameSite {
                a: "Johnson job".into(),
                b: "1420 Johnson Street".into(),
            },
            Answer::No,
        );
        let report = s.report_model_off();
        let text = text(&s, &report, &["Version 0.1.0".into()]);
        for name in [
            "Michael",
            "Baker",
            "Mike B",
            "José",
            "García",
            "Luis",
            "Ortiz",
            "Dave",
            "Kim",
            "Ana",
            "Ruiz",
            "Sam",
            "Patel",
            "Johnson",
            "Smith",
            "Parker",
            "Oak Ridge",
            "Shop",
        ] {
            assert!(word_matches(&text, name).is_empty(), "{name} in:\n{text}");
        }
        assert!(text.contains("Person 1"));
        assert!(text.contains("Site 1"));
    }
}
