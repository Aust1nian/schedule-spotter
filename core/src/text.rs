//! Small text helpers shared by every stage.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Trims and collapses every run of whitespace (including line breaks and
/// non-breaking spaces) to a single space.
pub fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() || ch == '\u{a0}' {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(ch);
        }
    }
    out
}

/// Removes accents: "José" → "Jose".
pub fn fold_accents(text: &str) -> String {
    text.nfd().filter(|c| !is_combining_mark(*c)).collect()
}

/// The comparison key for words: lowercase, accents removed, whitespace
/// collapsed. "  JOSÉ  García " → "jose garcia".
pub fn key(text: &str) -> String {
    fold_accents(&clean(text)).to_lowercase()
}

/// Lowercase alphanumeric tokens of `text`, accents removed. Apostrophes
/// inside words are dropped ("Lowe's" → "lowes"); every other non-letter,
/// non-digit splits.
pub fn tokens(text: &str) -> Vec<String> {
    let folded = key(text);
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in folded.chars() {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else if ch == '\'' || ch == '’' {
            // keep "lowes" together
        } else if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// True when every letter in `text` is uppercase and there are at least two.
pub fn is_shouting(text: &str) -> bool {
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_and_keys() {
        assert_eq!(clean("  Mike \t  Baker\n"), "Mike Baker");
        assert_eq!(clean("a\u{a0}b"), "a b");
        assert_eq!(key("  JOSÉ  García "), "jose garcia");
        assert_eq!(
            tokens("Lowe's Plaza T.I."),
            vec!["lowes", "plaza", "t", "i"]
        );
        assert_eq!(tokens("1420 Johnson St."), vec!["1420", "johnson", "st"]);
    }
}
