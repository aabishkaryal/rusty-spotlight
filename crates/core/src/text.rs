//! Text folding used for matching: case-insensitive and diacritic-insensitive.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Lowercase and strip combining marks ("Café" → "cafe"). Whitespace is collapsed.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_space = true;
    for c in s.nfd().filter(|c| !is_combining_mark(*c)) {
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else {
            out.extend(c.to_lowercase());
            last_space = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Split a title into folded words: on whitespace and punctuation, and at
/// lower→upper case changes ("TextEdit" → ["text", "edit"], "VS Code" → ["vs", "code"]).
pub fn words(title: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for c in title.nfd().filter(|c| !is_combining_mark(*c)) {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower && !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
        prev_lower = c.is_lowercase() || c.is_numeric();
        cur.extend(c.to_lowercase());
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_case_and_diacritics() {
        assert_eq!(fold("  Café   Señor "), "cafe senor");
    }

    #[test]
    fn splits_words() {
        assert_eq!(words("TextEdit"), ["text", "edit"]);
        assert_eq!(words("Visual Studio Code"), ["visual", "studio", "code"]);
        assert_eq!(words("T3 Code (Beta)"), ["t3", "code", "beta"]);
        assert_eq!(words("WhatsApp"), ["whats", "app"]);
    }
}
