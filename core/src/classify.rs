//! How much a value looks like a person's name or a job site. Layout
//! detection compares these scores over whole columns and grids, so single
//! misreads wash out.

use std::sync::LazyLock;

use regex::Regex;

use crate::lexicon::{
    JOB_WORDS, STREET_SUFFIXES, crew_name, is_company_token, is_first_name, is_internal_location,
    is_legal_suffix, is_placeholder, is_surname, is_travel, is_unstaffed_marker, part_word,
    status_word,
};
use crate::text::{key, tokens};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Employee,
    Placeholder,
    Subcontractor,
}

static HOUSE_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{1,6}[a-z]?\s+[a-z]").expect("regex"));
static JOB_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(^|\s)(j|job|#|no\.?|wo|po)\s*-?\s*#?\d{2,}|^\d{2,}-\d{2,}(\s|$)|^#\d+")
        .expect("regex")
});

/// Words that carry no identity on their own.
fn is_noise_word(k: &str) -> bool {
    status_word(k).is_some() || is_unstaffed_marker(k) || part_word(k).is_some()
}

pub fn is_company(text: &str) -> bool {
    let toks = tokens(text);
    if toks.iter().any(|t| is_legal_suffix(t)) && toks.len() >= 2 {
        return true;
    }
    toks.len() >= 2
        && toks.last().is_some_and(|t| is_company_token(t))
        && !is_first_name(&toks[0])
        && !toks
            .iter()
            .any(|t| JOB_WORDS.contains(t.as_str()) && !is_company_token(t))
}

pub fn entity_kind(text: &str) -> EntityKind {
    let k = key(text);
    if is_placeholder(&k) {
        EntityKind::Placeholder
    } else if is_company(text) {
        EntityKind::Subcontractor
    } else {
        EntityKind::Employee
    }
}

/// 0–1: how much `text` looks like a person (or a placeholder, crew or
/// subcontractor standing where a person would).
pub fn person_score(text: &str) -> f32 {
    let k = key(text);
    if k.is_empty() || k.len() > 40 || is_noise_word(&k) {
        return 0.0;
    }
    if is_internal_location(&k) || is_travel(&k) {
        return 0.0;
    }
    if is_placeholder(&k) {
        return 0.8;
    }
    if crew_name(&k) {
        return 0.6;
    }
    if is_company(text) {
        return 0.7;
    }
    let toks = tokens(text);
    if toks.is_empty() || toks.len() > 4 {
        return 0.05;
    }
    if toks.iter().any(|t| t.chars().any(|c| c.is_ascii_digit())) {
        return 0.05;
    }
    if HOUSE_NUMBER.is_match(&k) || JOB_NUMBER.is_match(&k) {
        return 0.05;
    }
    let job_like = toks
        .iter()
        .skip(1)
        .any(|t| STREET_SUFFIXES.contains(t.as_str()))
        || toks
            .iter()
            .any(|t| JOB_WORDS.contains(t.as_str()) && !is_first_name(t) && !is_surname(t));
    if job_like {
        return 0.1;
    }
    let first = toks[0].as_str();
    if is_first_name(first) {
        return if toks.len() == 1 { 0.85 } else { 0.95 };
    }
    if first.len() == 1 && toks.len() >= 2 && (is_surname(&toks[1]) || toks[1].len() > 2) {
        return 0.85; // "M. Baker"
    }
    if toks.len() >= 2 && is_surname(&toks[toks.len() - 1]) {
        return 0.7;
    }
    if toks.len() == 1 && is_surname(first) {
        return 0.45;
    }
    if text
        .chars()
        .all(|c| c.is_alphabetic() || c == ' ' || c == '.' || c == '\'' || c == '-')
    {
        return 0.4;
    }
    0.1
}

/// 0–1: how much `text` looks like a job site, job name or company place.
pub fn site_score(text: &str) -> f32 {
    let k = key(text);
    if k.is_empty() || is_noise_word(&k) {
        return 0.0;
    }
    if is_internal_location(&k) || is_travel(&k) {
        return 0.95;
    }
    if HOUSE_NUMBER.is_match(&k) {
        return 0.95;
    }
    if JOB_NUMBER.is_match(&k) {
        return 0.9;
    }
    let toks = tokens(text);
    if toks.is_empty() {
        return 0.0;
    }
    if toks
        .iter()
        .skip(1)
        .any(|t| STREET_SUFFIXES.contains(t.as_str()))
    {
        return 0.85;
    }
    if toks.iter().any(|t| JOB_WORDS.contains(t.as_str())) {
        return 0.85;
    }
    if text.contains("'s") || text.contains("’s") || text.ends_with("s'") {
        return 0.7;
    }
    if is_placeholder(&k) || crew_name(&k) {
        return 0.05;
    }
    if is_company(text) {
        return 0.3;
    }
    if toks.iter().any(|t| t.chars().any(|c| c.is_ascii_digit())) {
        return 0.5;
    }
    let first = toks[0].as_str();
    if is_first_name(first) && !is_surname(first) {
        return if toks.len() == 1 { 0.1 } else { 0.15 };
    }
    if toks.len() == 1 && is_surname(first) {
        return if is_first_name(first) { 0.3 } else { 0.45 };
    }
    if is_first_name(first) && toks.len() >= 2 {
        return 0.15;
    }
    0.4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn people_and_sites() {
        for p in [
            "Mike Baker",
            "Mike B",
            "M. Baker",
            "José García",
            "Chuy",
            "Helper 1",
        ] {
            assert!(
                person_score(p) > site_score(p),
                "{p}: person {} site {}",
                person_score(p),
                site_score(p)
            );
        }
        for s in [
            "1420 Johnson St",
            "Smith remodel",
            "J-1042 Oak Ridge",
            "Shop",
            "Lowe's Plaza TI",
            "drive to Fresno",
            "Oak Ridge Apts",
        ] {
            assert!(
                site_score(s) > person_score(s),
                "{s}: person {} site {}",
                person_score(s),
                site_score(s)
            );
        }
        assert_eq!(entity_kind("Delta Electric LLC"), EntityKind::Subcontractor);
        assert_eq!(entity_kind("ABC Plumbing"), EntityKind::Subcontractor);
        assert_eq!(entity_kind("Laborer 2"), EntityKind::Placeholder);
        assert_eq!(entity_kind("Mike Baker"), EntityKind::Employee);
    }
}
