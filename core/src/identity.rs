//! Who and where, as identities rather than spellings.
//!
//! Spellings that differ only in case, spacing or trailing punctuation are
//! one identity without asking. For sites, street-suffix spellings
//! ("Street"/"St") are too. Anything looser (nicknames, initials, typos,
//! near-identical addresses) is matching (stage 7), which shows every merge
//! it makes.

use std::collections::HashMap;

use crate::text::{clean, key, tokens};

/// A person's identity key: lowercase, whitespace collapsed, stray end
/// punctuation dropped. Accents are kept: "José" and "Jose" may be two
/// people until matching says otherwise.
pub fn person_key(name: &str) -> String {
    clean(name)
        .trim_matches(|c: char| c == '.' || c == ',' || c == '*' || c == '-')
        .to_lowercase()
}

fn suffix(token: &str) -> &str {
    match token {
        "street" => "st",
        "avenue" | "av" => "ave",
        "road" => "rd",
        "drive" => "dr",
        "lane" => "ln",
        "court" => "ct",
        "boulevard" => "blvd",
        "place" => "pl",
        "circle" => "cir",
        "parkway" => "pkwy",
        "highway" => "hwy",
        "terrace" => "ter",
        "trail" => "trl",
        "square" => "sq",
        "north" => "n",
        "south" => "s",
        "east" => "e",
        "west" => "w",
        "apartments" => "apts",
        "apartment" => "apt",
        "building" => "bldg",
        "suite" => "ste",
        "mount" => "mt",
        other => other,
    }
}

/// A site's identity key: tokens, accents removed, street suffixes and
/// directions abbreviated, "the" dropped. "1420 Johnson Street." and
/// "1420 johnson st" share a key; "1240 Johnson St" does not.
pub fn site_key(site: &str) -> String {
    tokens(site).iter().filter(|t| t.as_str() != "the").map(|t| suffix(t)).collect::<Vec<_>>().join(" ")
}

/// Groups spellings by key and names each group by its longest spelling
/// (ties: the one seen first).
#[derive(Default, Debug)]
pub struct Names {
    display: HashMap<String, String>,
}

impl Names {
    pub fn add(&mut self, key: &str, spelling: &str) {
        let spelling = clean(spelling);
        let entry = self.display.entry(key.to_string()).or_insert_with(|| spelling.clone());
        if spelling.chars().count() > entry.chars().count() {
            *entry = spelling;
        }
    }

    pub fn display(&self, key: &str) -> String {
        self.display.get(key).cloned().unwrap_or_else(|| key.to_string())
    }
}

/// The comparison key for "is this the company's own place".
pub fn is_internal(site: &str) -> bool {
    let k = key(site);
    crate::lexicon::is_internal_location(&k) || crate::lexicon::is_travel(&k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys() {
        assert_eq!(person_key(" MIKE  Baker. "), "mike baker");
        assert_ne!(person_key("José"), person_key("Jose"));
        assert_eq!(site_key("1420 Johnson Street."), site_key("1420 johnson st"));
        assert_ne!(site_key("1420 Johnson St"), site_key("1240 Johnson St"));
        assert_eq!(site_key("The Shop"), site_key("shop"));
        let mut names = Names::default();
        names.add("k", "1420 Johnson St");
        names.add("k", "1420 Johnson Street");
        assert_eq!(names.display("k"), "1420 Johnson Street");
    }
}
