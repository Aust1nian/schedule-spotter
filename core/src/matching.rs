//! Stage 7: which spellings are the same person or the same site.
//!
//! Order of authority: these deterministic rules first, the model second
//! (Phase 4, through [`Tiebreaker`]), the owner last, and the owner's
//! answer overrides everything, including undoing a rule's merge. Every
//! merge is recorded so the app can show it as a "treating A as B" chip.
//!
//! Candidate pairs come only from similarity (Jaro-Winkler ≥ 0.85, a shared
//! token, or compatible initials), never from all pairs blindly.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::classify::{EntityKind, entity_kind};
use crate::identity::{Names, is_internal, person_key, site_key};
use crate::lexicon::{JOB_WORDS, NICKNAMES, STREET_SUFFIXES, is_legal_suffix};
use crate::model::{Answer, Answers, CellRef, Question, Reading};
use crate::text::tokens;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeKind {
    Person,
    Site,
}

/// Why two spellings were treated as one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeReason {
    /// Case, spacing, accents or punctuation ("Jose" / "José").
    Spelling,
    /// Nickname table ("Mike" / "Michael").
    Nickname,
    /// Initials, or a first or last name alone, that fit exactly one person.
    OnlyMatch,
    /// Street suffix or direction spelling ("St" / "Street").
    Address,
    /// A job number in front of the same job name.
    JobNumber,
    /// The model said so (Phase 4).
    Model,
    /// The owner said so.
    Answered,
}

/// "Treating `from` as `into`."
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Merge {
    pub kind: MergeKind,
    pub from: String,
    pub into: String,
    pub reason: MergeReason,
}

/// What the model says about one closed question (Phase 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Same,
    Different,
    Unsure,
}

/// One record read from a free-text line by the model (task T5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEntry {
    pub person: String,
    /// mon … sun
    pub day: String,
    pub site: String,
    /// "HH:MM-HH:MM", when the line gives one.
    #[serde(default)]
    pub time: Option<String>,
}

/// The model layer, reached only through this: single closed questions the
/// rules can't settle (spec tasks T1–T5). Every default answer is "unsure",
/// which sends the question to the owner.
pub trait Tiebreaker {
    /// T1: are these two spellings one person?
    fn same_person(&self, _a: &str, _b: &str, _roster: &[String]) -> Verdict {
        Verdict::Unsure
    }
    /// T2: are these two sites one place?
    fn same_site(&self, _a: &str, _b: &str, _sites: &[String]) -> Verdict {
        Verdict::Unsure
    }
    /// T3: what is this cell (person, site, status, unstaffed, note, time, empty)?
    fn cell_meaning(
        &self,
        _text: &str,
        _column_role: &str,
        _neighbors: &[String],
    ) -> Option<String> {
        None
    }
    /// T4: what role does this column play, when the script's scores tie?
    fn column_role(&self, _header: &str, _samples: &[String]) -> Option<String> {
        None
    }
    /// T5: the records in one pasted free-text line, or None ("cannot read").
    fn free_text(
        &self,
        _line: &str,
        _roster: &[String],
        _sites: &[String],
    ) -> Option<Vec<TextEntry>> {
        None
    }
    /// Whether a model is loaded at all (free text needs one).
    fn available(&self) -> bool {
        false
    }
}

/// Model off: everything unsure goes to the owner.
pub struct NoModel;
impl Tiebreaker for NoModel {}

/// The outcome: identity of every spelling, the merges, and the questions.
#[derive(Debug, Default)]
pub struct Matching {
    person_of: HashMap<String, String>,
    site_of: HashMap<String, String>,
    /// Site spellings the owner split from the rest of their key ("no" to
    /// a "1420 Johnson St" / "1420 Johnson Street" chip): spelling → key.
    site_split: HashMap<String, String>,
    pub people: Names,
    pub sites: Names,
    pub merges: Vec<Merge>,
    pub questions: Vec<Question>,
}

impl Matching {
    /// The identity key of a person spelling.
    pub fn person(&self, raw: &str) -> String {
        let k = person_key(raw);
        self.person_of.get(&k).cloned().unwrap_or(k)
    }

    /// The identity key of a site spelling.
    pub fn site(&self, raw: &str) -> String {
        let k = self
            .site_split
            .get(&crate::text::clean(raw))
            .cloned()
            .unwrap_or_else(|| site_key(raw));
        self.site_of.get(&k).cloned().unwrap_or(k)
    }
}

// ------------------------------------------------------------- similarity

/// Jaro-Winkler similarity, 0–1.
pub fn jaro_winkler(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() || b.is_empty() {
        return if a.is_empty() && b.is_empty() {
            1.0
        } else {
            0.0
        };
    }
    let window = (a.len().max(b.len()) / 2).saturating_sub(1);
    let mut a_match = vec![false; a.len()];
    let mut b_match = vec![false; b.len()];
    let mut matches = 0usize;
    for i in 0..a.len() {
        let lo = i.saturating_sub(window);
        let hi = (i + window + 1).min(b.len());
        for j in lo..hi {
            if !b_match[j] && a[i] == b[j] {
                a_match[i] = true;
                b_match[j] = true;
                matches += 1;
                break;
            }
        }
    }
    if matches == 0 {
        return 0.0;
    }
    let mut transpositions = 0usize;
    let mut k = 0usize;
    for i in 0..a.len() {
        if a_match[i] {
            while !b_match[k] {
                k += 1;
            }
            if a[i] != b[k] {
                transpositions += 1;
            }
            k += 1;
        }
    }
    let m = matches as f64;
    let jaro =
        (m / a.len() as f64 + m / b.len() as f64 + (m - transpositions as f64 / 2.0) / m) / 3.0;
    let prefix = a.iter().zip(&b).take(4).take_while(|(x, y)| x == y).count() as f64;
    jaro + prefix * 0.1 * (1.0 - jaro)
}

// ------------------------------------------------------------------ people

#[derive(Clone, Debug)]
struct PersonName {
    first: String,
    rest: Vec<String>,
    generation: Option<String>,
    folded: String,
    kind: EntityKind,
}

fn parse_person(display: &str) -> PersonName {
    // "Baker, Mike" → "Mike Baker".
    let reordered = match display.split_once(',') {
        Some((last, first)) if !first.trim().is_empty() && !last.trim().contains(' ') => {
            format!("{} {}", first.trim(), last.trim())
        }
        _ => display.to_string(),
    };
    let mut toks = tokens(&reordered);
    let generation = toks
        .iter()
        .position(|t| matches!(t.as_str(), "jr" | "sr" | "ii" | "iii" | "iv"))
        .map(|i| toks.remove(i));
    toks.retain(|t| !is_legal_suffix(t));
    let folded = toks.join(" ");
    let first = toks.first().cloned().unwrap_or_default();
    let rest = toks.iter().skip(1).cloned().collect();
    PersonName {
        first,
        rest,
        generation,
        folded,
        kind: entity_kind(display),
    }
}

fn nick_forms(name: &str) -> BTreeSet<&str> {
    let mut out = BTreeSet::new();
    if let Some(full) = NICKNAMES.get(name) {
        out.extend(full.iter().copied());
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FirstFit {
    /// Same first name, or nickname-equivalent ("Mike" / "Michael").
    Full,
    /// `a` has only the initial ("M." / "Michael").
    AInitial,
    /// `b` has only the initial.
    BInitial,
}

/// Mike ~ Michael, Bob ~ Rob (both Robert), M ~ Michael. An initial fits a
/// real first name, or any first name followed by a surname: "S." is not
/// "Swanson", but "R. Singh" may be "Ravi Singh".
fn first_names_fit(a: &PersonName, b: &PersonName) -> Option<FirstFit> {
    if a.first == b.first {
        return Some(FirstFit::Full);
    }
    // "R" fits "Robert", and "Bob" too (Bob is Robert).
    let initial_of = |i: &PersonName, full: &PersonName| {
        let starts = full.first.starts_with(i.first.as_str())
            || nick_forms(&full.first)
                .iter()
                .any(|f| f.starts_with(i.first.as_str()));
        i.first.len() == 1
            && full.first.len() > 1
            && starts
            && (crate::lexicon::is_first_name(&full.first) || !full.rest.is_empty())
    };
    if initial_of(a, b) {
        return Some(FirstFit::AInitial);
    }
    if initial_of(b, a) {
        return Some(FirstFit::BInitial);
    }
    let (na, nb) = (nick_forms(&a.first), nick_forms(&b.first));
    if na.contains(b.first.as_str()) || nb.contains(a.first.as_str()) || !na.is_disjoint(&nb) {
        return Some(FirstFit::Full);
    }
    None
}

/// How much of a name is spelled out: full words count 2, initials 1.
fn specificity(p: &PersonName) -> u32 {
    std::iter::once(&p.first)
        .chain(&p.rest)
        .map(|t| if t.len() > 1 { 2 } else { 1 })
        .sum()
}

#[derive(Debug, PartialEq)]
enum Fit {
    /// Same name up to spelling or nickname.
    Exact,
    /// One is a less specific spelling (initials, a part missing).
    Partial,
    /// Could be the same, can't say (typo, a dropped second surname).
    Unsure,
    Different,
}

fn surnames_fit(a: &[String], b: &[String]) -> Fit {
    if a.is_empty() || b.is_empty() {
        return if a.is_empty() && b.is_empty() {
            Fit::Exact
        } else {
            Fit::Partial
        };
    }
    if a == b {
        return Fit::Exact;
    }
    // "B" or "B." against "Baker".
    let initial = |x: &[String], y: &[String]| {
        x.len() == 1 && x[0].len() == 1 && y[0].starts_with(x[0].as_str())
    };
    if initial(a, b) || initial(b, a) {
        return Fit::Partial;
    }
    // José García vs José García López: one surname list extends the other.
    let (short, long) = if a.len() < b.len() { (a, b) } else { (b, a) };
    if long.starts_with(short) {
        return Fit::Unsure;
    }
    if a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x == y || jaro_winkler(x, y) >= 0.9)
    {
        return Fit::Unsure; // a typo in a surname
    }
    Fit::Different
}

fn person_fit(a: &PersonName, b: &PersonName) -> Fit {
    // Sr and Jr are two people, however alike the rest.
    if a.generation.is_some() && b.generation.is_some() && a.generation != b.generation {
        return Fit::Different;
    }
    if a.folded == b.folded {
        return Fit::Exact;
    }
    if a.kind != b.kind || a.kind != EntityKind::Employee {
        return Fit::Different;
    }
    // A lone first name may be anyone with that first name: ask. (A lone
    // last name that fits one person is safer; it merges below.)
    let lone = a.rest.is_empty() || b.rest.is_empty();
    match first_names_fit(a, b) {
        Some(first) => {
            let surnames = surnames_fit(&a.rest, &b.rest);
            match (first, surnames) {
                (_, Fit::Different) => Fit::Different,
                (_, Fit::Unsure) => Fit::Unsure,
                _ if lone => Fit::Unsure,
                (FirstFit::Full, Fit::Exact) => Fit::Exact,
                // One side is less specific in every part it differs:
                // "Mike B" / "Michael Baker", "M. Baker" / "Michael Baker".
                // If each side spells out what the other abbreviates
                // ("R Singh" / "Robert S"), it's a question.
                _ if specificity(a) == specificity(b) => Fit::Unsure,
                _ => {
                    let (less, more) = if specificity(a) < specificity(b) {
                        (a, b)
                    } else {
                        (b, a)
                    };
                    let initial_first_on_more = match first {
                        FirstFit::AInitial => std::ptr::eq(a, more),
                        FirstFit::BInitial => std::ptr::eq(b, more),
                        FirstFit::Full => false,
                    };
                    let shorter_surname_on_more =
                        surname_specificity(&more.rest) < surname_specificity(&less.rest);
                    if initial_first_on_more || shorter_surname_on_more {
                        Fit::Unsure
                    } else {
                        Fit::Partial
                    }
                }
            }
        }
        None => {
            // A last name alone ("Baker" / "Michael Baker"), or a typo.
            let last_only = |x: &PersonName, y: &PersonName| {
                x.rest.is_empty() && y.rest.last() == Some(&x.first)
            };
            let typo = jaro_winkler(&a.first, &b.first) >= 0.85
                && matches!(surnames_fit(&a.rest, &b.rest), Fit::Exact | Fit::Partial);
            if last_only(a, b) || last_only(b, a) {
                Fit::Partial // merges only if one person has that surname
            } else if typo || (lone && jaro_winkler(&a.folded, &b.folded) >= 0.85) {
                Fit::Unsure
            } else {
                Fit::Different
            }
        }
    }
}

fn surname_specificity(rest: &[String]) -> u32 {
    rest.iter().map(|t| if t.len() > 1 { 2 } else { 1 }).sum()
}

/// Worth comparing at all: similar strings, a shared token, or initials
/// that agree.
fn person_candidates(a: &PersonName, b: &PersonName) -> bool {
    jaro_winkler(&a.folded, &b.folded) >= 0.85
        || a.rest
            .iter()
            .chain(std::iter::once(&a.first))
            .any(|t| t.len() > 1 && (b.first == *t || b.rest.contains(t)))
        || a.first.chars().next() == b.first.chars().next()
        || !nick_forms(&a.first).is_disjoint(&nick_forms(&b.first))
        || nick_forms(&a.first).contains(b.first.as_str())
        || nick_forms(&b.first).contains(a.first.as_str())
}

// ------------------------------------------------------------------- sites

#[derive(Clone, Debug)]
struct SiteName {
    key: String,
    house: Option<String>,
    job_no: Option<String>,
    /// Words without house or job numbers.
    words: Vec<String>,
    internal: bool,
}

fn is_job_number(t: &str) -> bool {
    t.chars().any(|c| c.is_ascii_digit())
        && t.chars().any(|c| c.is_ascii_alphabetic())
        && t.len() <= 8
}

fn parse_site(display: &str) -> SiteName {
    let key = site_key(display);
    let toks: Vec<String> = key
        .split(' ')
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect();
    let mut house = None;
    let mut job_no = None;
    let mut words = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let all_digits = t.chars().all(|c| c.is_ascii_digit());
        if i == 0 && all_digits && toks.len() > 1 && !display.trim_start().starts_with('#') {
            house = Some(t.clone());
        } else if t == "j"
            || t == "job"
                && toks
                    .get(i + 1)
                    .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()))
        {
            continue;
        } else if all_digits || is_job_number(t) {
            job_no = Some(t.clone());
        } else {
            words.push(t.clone());
        }
    }
    SiteName {
        key,
        house,
        job_no,
        words,
        internal: is_internal(display),
    }
}

fn generic_word(t: &str) -> bool {
    STREET_SUFFIXES.contains(t)
        || JOB_WORDS.contains(t)
        || matches!(
            t,
            "n" | "s" | "e" | "w" | "the" | "and" | "of" | "at" | "on" | "a"
        )
        || t.len() <= 2
}

/// Same, different or unsure, with the reason when same.
fn site_fit(a: &SiteName, b: &SiteName) -> (Fit, MergeReason) {
    if a.internal != b.internal {
        return (Fit::Different, MergeReason::Address);
    }
    if a.house.is_some() && a.house == b.house && a.words == b.words {
        return (Fit::Exact, MergeReason::Address);
    }
    if a.job_no.is_some() && b.job_no.is_some() && a.job_no != b.job_no {
        return (Fit::Different, MergeReason::JobNumber); // Walgreens #3299 vs #1087
    }
    if a.job_no.is_some() != b.job_no.is_some()
        && a.house == b.house
        && !a.words.is_empty()
        && a.words == b.words
    {
        return (Fit::Exact, MergeReason::JobNumber);
    }
    let distinctive = |s: &SiteName| {
        s.words
            .iter()
            .filter(|t| !generic_word(t))
            .cloned()
            .collect::<BTreeSet<_>>()
    };
    let (da, db) = (distinctive(a), distinctive(b));
    let shared = !da.is_disjoint(&db);
    match (&a.house, &b.house) {
        // 1420 vs 1240 Johnson St: never merged, always asked.
        (Some(x), Some(y)) if x != y => {
            if shared || a.words == b.words {
                (Fit::Unsure, MergeReason::Address)
            } else {
                (Fit::Different, MergeReason::Address)
            }
        }
        _ => {
            if shared || jaro_winkler(&a.key, &b.key) >= 0.92 {
                (Fit::Unsure, MergeReason::Address)
            } else {
                (Fit::Different, MergeReason::Address)
            }
        }
    }
}

// ---------------------------------------------------------------- matching

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> UnionFind {
        UnionFind {
            parent: (0..n).collect(),
        }
    }
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.parent[r] != r {
            r = self.parent[r];
        }
        let mut y = x;
        while self.parent[y] != r {
            let next = self.parent[y];
            self.parent[y] = r;
            y = next;
        }
        r
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[rb] = ra;
        }
    }
}

fn answered(answers: &Answers, q: &Question) -> Option<bool> {
    answers.get(q).and_then(|a| match a {
        Answer::Yes => Some(true),
        Answer::No => Some(false),
        Answer::Pick(_) => None,
    })
}

/// Matches people and sites across a reading.
pub fn match_entities(reading: &Reading, answers: &Answers, tiebreak: &dyn Tiebreaker) -> Matching {
    let mut m = Matching::default();
    match_people(reading, answers, tiebreak, &mut m);
    match_sites(reading, answers, tiebreak, &mut m);
    m
}

/// What the rules decided about the spellings of one kind (people or sites).
#[derive(Default)]
struct Decisions {
    /// Same by rule, with the reason.
    exact: Vec<(usize, usize, MergeReason)>,
    /// Less specific spelling → the spellings it could be.
    partial: BTreeMap<usize, Vec<usize>>,
    unsure: Vec<(usize, usize)>,
}

/// Turns rule decisions, answers and the model into groups, merges and
/// questions. The owner can undo any rule merge (answering "no" to its chip,
/// `same_person {from, into}`); questions name each side by its group's
/// canonical spelling.
struct Resolved {
    root: Vec<usize>,
    display: Vec<String>,
    merges: Vec<Merge>,
    questions: Vec<Question>,
}

fn resolve(
    kind: MergeKind,
    names: &[String],
    d: Decisions,
    answers: &Answers,
    model: &dyn Fn(&str, &str) -> Verdict,
) -> Resolved {
    let n = names.len();
    let ask = |a: &str, b: &str| match kind {
        MergeKind::Person => Question::SamePerson {
            a: a.to_string(),
            b: b.to_string(),
        },
        MergeKind::Site => Question::SameSite {
            a: a.to_string(),
            b: b.to_string(),
        },
    };
    let said = |a: &str, b: &str| answered(answers, &ask(a, b));
    let display_of = |uf: &mut UnionFind| -> Vec<String> {
        let mut best: HashMap<usize, String> = HashMap::new();
        for (i, name) in names.iter().enumerate() {
            let r = uf.find(i);
            let e = best.entry(r).or_insert_with(|| name.clone());
            if better_spelling(name, e) {
                *e = name.clone();
            }
        }
        (0..n).map(|i| best[&uf.find(i)].clone()).collect()
    };

    // Rule merges, first without and then with the owner's undos.
    let rule_unions = |uf: &mut UnionFind,
                       undone: &BTreeSet<(usize, usize)>|
     -> Vec<(usize, usize, MergeReason)> {
        let mut done = Vec::new();
        for &(i, j, reason) in &d.exact {
            if !undone.contains(&(i, j)) {
                uf.union(i, j);
                done.push((i, j, reason));
            }
        }
        for (less, mores) in &d.partial {
            let mut groups: Vec<usize> = mores.iter().map(|x| uf.find(*x)).collect();
            groups.sort_unstable();
            groups.dedup();
            if groups.len() == 1 && !undone.contains(&(*less, mores[0])) {
                uf.union(groups[0], *less);
                done.push((*less, mores[0], MergeReason::OnlyMatch));
            }
        }
        done
    };
    let mut first = UnionFind::new(n);
    let tentative = rule_unions(&mut first, &BTreeSet::new());
    let shown = display_of(&mut first);
    let undone: BTreeSet<(usize, usize)> = tentative
        .iter()
        .filter(|(i, j, _)| {
            [(i, j), (j, i)].iter().any(|(x, y)| {
                said(&names[**x], &shown[**x]) == Some(false)
                    || said(&names[**x], &names[**y]) == Some(false)
            })
        })
        .map(|(i, j, _)| (*i, *j))
        .collect();
    let mut uf = UnionFind::new(n);
    let mut unions = rule_unions(&mut uf, &undone);

    // Unsure pairs (and partials that fit several groups) between groups.
    let mut pending: Vec<(usize, usize)> = d.unsure.clone();
    for (less, mores) in &d.partial {
        let mut groups: Vec<usize> = mores.iter().map(|x| uf.find(*x)).collect();
        groups.sort_unstable();
        groups.dedup();
        if groups.len() > 1 || undone.contains(&(*less, mores[0])) {
            pending.extend(mores.iter().map(|m| (*less, *m)));
        }
    }
    let mut questions = Vec::new();
    let mut seen: BTreeSet<(usize, usize)> = BTreeSet::new();
    for (i, j) in pending {
        let shown = display_of(&mut uf);
        let (gi, gj) = (uf.find(i), uf.find(j));
        if gi == gj || !seen.insert((gi.min(gj), gi.max(gj))) {
            continue;
        }
        let (a, b) = (&shown[i], &shown[j]);
        match said(a, b).or_else(|| said(&names[i], &names[j])) {
            Some(true) => {
                uf.union(i, j);
                unions.push((i, j, MergeReason::Answered));
                continue;
            }
            Some(false) => continue,
            None => {}
        }
        match model(a, b) {
            Verdict::Same => {
                uf.union(i, j);
                unions.push((i, j, MergeReason::Model));
            }
            Verdict::Different => {}
            Verdict::Unsure => {
                let q = ask(a, b);
                if !questions.contains(&q) {
                    questions.push(q);
                }
            }
        }
    }

    let display = display_of(&mut uf);
    let root: Vec<usize> = (0..n).map(|i| uf.find(i)).collect();
    let mut merges: Vec<Merge> = Vec::new();
    for (i, j, reason) in unions {
        for x in [i, j] {
            if names[x] != display[x] && !merges.iter().any(|g| g.from == names[x]) {
                merges.push(Merge {
                    kind,
                    from: names[x].clone(),
                    into: display[x].clone(),
                    reason,
                });
            }
        }
    }
    Resolved {
        root,
        display,
        merges,
        questions,
    }
}

/// The canonical spelling of a group: the longest; on a tie, the one with
/// accents ("José García" over "Jose Garcia"), then the first seen.
fn better_spelling(candidate: &str, current: &str) -> bool {
    let accents = |s: &str| s.chars().filter(|c| !c.is_ascii()).count();
    (candidate.chars().count(), accents(candidate)) > (current.chars().count(), accents(current))
}

fn match_people(reading: &Reading, answers: &Answers, tiebreak: &dyn Tiebreaker, m: &mut Matching) {
    // Identities by key, in first-seen order; and which identities share a
    // cell (two names in one cell are two people).
    let mut spellings: Names = Names::default();
    let mut order: Vec<String> = Vec::new();
    let mut cells: HashMap<CellRef, BTreeSet<String>> = HashMap::new();
    let mut all: Vec<&str> = reading
        .assignments
        .iter()
        .filter_map(|a| a.person.as_deref())
        .collect();
    if let Some(roster) = &reading.roster {
        all.extend(roster.iter().map(String::as_str));
    }
    for raw in &all {
        let k = person_key(raw);
        if !order.contains(&k) {
            order.push(k.clone());
        }
        spellings.add(&k, raw);
    }
    for a in &reading.assignments {
        if let Some(p) = &a.person {
            cells.entry(a.source).or_default().insert(person_key(p));
        }
    }
    let together: BTreeSet<(&str, &str)> = cells
        .values()
        .flat_map(|ks| {
            let ks: Vec<&String> = ks.iter().collect();
            let mut pairs = Vec::new();
            for i in 0..ks.len() {
                for j in 0..ks.len() {
                    if i != j {
                        pairs.push((ks[i].as_str(), ks[j].as_str()));
                    }
                }
            }
            pairs
        })
        .collect();

    let names: Vec<String> = order.iter().map(|k| spellings.display(k)).collect();
    let parsed: Vec<PersonName> = names.iter().map(|n| parse_person(n)).collect();
    let n = order.len();
    let mut d = Decisions::default();
    for i in 0..n {
        for j in i + 1..n {
            if together.contains(&(order[i].as_str(), order[j].as_str()))
                || !person_candidates(&parsed[i], &parsed[j])
            {
                continue;
            }
            match person_fit(&parsed[i], &parsed[j]) {
                Fit::Exact => {
                    let reason = if parsed[i].first == parsed[j].first {
                        MergeReason::Spelling
                    } else {
                        MergeReason::Nickname
                    };
                    d.exact.push((i, j, reason));
                }
                Fit::Partial => {
                    let less = if specificity(&parsed[i]) < specificity(&parsed[j]) {
                        i
                    } else {
                        j
                    };
                    let more = if less == i { j } else { i };
                    d.partial.entry(less).or_default().push(more);
                }
                Fit::Unsure => d.unsure.push((i, j)),
                Fit::Different => {}
            }
        }
    }
    let roster = names.clone();
    let r = resolve(MergeKind::Person, &names, d, answers, &|a, b| {
        tiebreak.same_person(a, b, &roster)
    });
    for i in 0..n {
        m.person_of
            .insert(order[i].clone(), order[r.root[i]].clone());
        m.people.set(&order[r.root[i]], &r.display[i]);
    }
    m.merges.extend(r.merges);
    m.questions.extend(r.questions);
}

fn match_sites(reading: &Reading, answers: &Answers, tiebreak: &dyn Tiebreaker, m: &mut Matching) {
    let mut spellings = Names::default();
    let mut order: Vec<String> = Vec::new();
    let mut raw_by_key: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let raws: Vec<&str> = reading
        .assignments
        .iter()
        .filter_map(|a| a.site.as_deref())
        .chain(reading.grid_jobs.iter().map(String::as_str))
        .collect();
    // Spellings that share a key ("St" / "Street") are one site, unless the
    // owner undid that chip: then the spelling gets a key of its own.
    let mut by_key: BTreeMap<String, Names> = BTreeMap::new();
    let mut seen_raw: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for raw in &raws {
        let k = site_key(raw);
        by_key.entry(k.clone()).or_default().add(&k, raw);
        seen_raw
            .entry(k)
            .or_default()
            .insert(crate::text::clean(raw));
    }
    for (k, group) in &seen_raw {
        let display = by_key[k].display(k);
        for raw in group {
            // The display spelling stays; the other side is split off.
            let undone = *raw != display
                && group.iter().any(|other| {
                    other != raw
                        && answered(
                            answers,
                            &Question::SameSite {
                                a: raw.clone(),
                                b: other.clone(),
                            },
                        ) == Some(false)
                });
            if undone {
                m.site_split.insert(raw.clone(), format!("{k}\u{1}{raw}"));
            }
        }
    }
    for raw in raws {
        let k = m
            .site_split
            .get(&crate::text::clean(raw))
            .cloned()
            .unwrap_or_else(|| site_key(raw));
        if !order.contains(&k) {
            order.push(k.clone());
        }
        spellings.add(&k, raw);
        raw_by_key
            .entry(k)
            .or_default()
            .insert(crate::text::clean(raw));
    }
    let names: Vec<String> = order.iter().map(|k| spellings.display(k)).collect();
    let parsed: Vec<SiteName> = names.iter().map(|n| parse_site(n)).collect();
    let n = order.len();
    let mut d = Decisions::default();
    for i in 0..n {
        for j in i + 1..n {
            match site_fit(&parsed[i], &parsed[j]) {
                (Fit::Exact, reason) => d.exact.push((i, j, reason)),
                (Fit::Unsure, _) => d.unsure.push((i, j)),
                _ => {}
            }
        }
    }
    let r = resolve(MergeKind::Site, &names, d, answers, &|a, b| {
        tiebreak.same_site(a, b, &names)
    });
    for i in 0..n {
        m.site_of.insert(order[i].clone(), order[r.root[i]].clone());
        m.sites.set(&order[r.root[i]], &r.display[i]);
    }
    // Spellings within one key ("St" / "Street") are merges too: their chip.
    for (k, raws) in &raw_by_key {
        let i = order.iter().position(|o| o == k).expect("key seen");
        for raw in raws {
            if *raw != r.display[i]
                && !m
                    .merges
                    .iter()
                    .any(|g| g.kind == MergeKind::Site && &g.from == raw)
            {
                m.merges.push(Merge {
                    kind: MergeKind::Site,
                    from: raw.clone(),
                    into: r.display[i].clone(),
                    reason: MergeReason::Address,
                });
            }
        }
    }
    for g in r.merges {
        if !m
            .merges
            .iter()
            .any(|x| x.kind == g.kind && x.from == g.from)
        {
            m.merges.push(g);
        }
    }
    m.questions.extend(r.questions);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fit(a: &str, b: &str) -> Fit {
        person_fit(&parse_person(a), &parse_person(b))
    }

    #[test]
    fn jaro() {
        assert!((jaro_winkler("martha", "marhta") - 0.961).abs() < 0.01);
        assert!(jaro_winkler("micheal", "michael") >= 0.85);
    }

    #[test]
    fn people() {
        assert_eq!(fit("José García", "Jose Garcia"), Fit::Exact);
        assert_eq!(fit("Mike Baker", "Michael Baker"), Fit::Exact);
        assert_eq!(fit("Pepe García", "José García"), Fit::Exact);
        assert_eq!(fit("Mike B", "Michael Baker"), Fit::Partial);
        assert_eq!(fit("M. Baker", "Michael Baker"), Fit::Partial);
        assert_eq!(fit("Baker", "Michael Baker"), Fit::Partial);
        assert_eq!(fit("Mike", "Michael Baker"), Fit::Unsure);
        assert_eq!(fit("R Singh", "Robert S"), Fit::Unsure);
        assert_eq!(fit("R Singh", "Ravi Singh"), Fit::Partial);
        assert_eq!(fit("Swanson", "S. Ingram"), Fit::Different);
        assert_eq!(fit("Jeff", "Jeffrey"), Fit::Unsure);
        assert_eq!(fit("Mike B", "Mike R"), Fit::Different);
        assert_eq!(fit("Bob Smith Sr", "Bob Smith Jr"), Fit::Different);
        assert_eq!(fit("Chris Lee", "Chris Ortiz"), Fit::Different);
        assert_eq!(fit("Micheal Baker", "Michael Baker"), Fit::Unsure);
        assert_eq!(fit("José García López", "José García"), Fit::Unsure);
        assert_eq!(fit("Helper 1", "Helper 2"), Fit::Different);
        assert_eq!(fit("Baker, Mike", "Mike Baker"), Fit::Exact);
    }

    #[test]
    fn sites() {
        let s = |a: &str, b: &str| site_fit(&parse_site(a), &parse_site(b)).0;
        assert_eq!(s("1420 Johnson St", "1240 Johnson St"), Fit::Unsure);
        assert_eq!(s("Johnson St", "1420 Johnson St"), Fit::Unsure);
        assert_eq!(s("Johnson job", "1420 Johnson St"), Fit::Unsure);
        assert_eq!(s("J-1042 Smith remodel", "Smith remodel"), Fit::Exact);
        assert_eq!(s("Smith remodel", "1420 Johnson St"), Fit::Different);
        assert_eq!(s("Shop", "Smith remodel"), Fit::Different);
    }
}
