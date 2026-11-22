//! Ranking (FR-11). The tier decides first; learned preference reorders only
//! inside a tier; the fuzzy matcher's score matters only inside the fuzzy tier.

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::text::{fold, words};
use crate::usage::Usage;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    App,
    Link,
}

#[derive(Clone, Debug)]
pub struct Item {
    /// Stable identity: an app's bundle path, or `link:<keyword>`.
    pub id: String,
    pub title: String,
    pub kind: Kind,
    pub aliases: Vec<String>,
    // Derived once, at catalog build time.
    folded: String,
    words: Vec<String>,
    acronym: String,
    folded_aliases: Vec<String>,
}

impl Item {
    pub fn new(id: impl Into<String>, title: impl Into<String>, kind: Kind, aliases: Vec<String>) -> Self {
        let title = title.into();
        let words = words(&title);
        let acronym = words.iter().filter_map(|w| w.chars().next()).collect();
        Self {
            id: id.into(),
            folded: fold(&title),
            folded_aliases: aliases.iter().map(|a| fold(a)).collect(),
            words,
            acronym,
            title,
            kind,
            aliases,
        }
    }
}

/// Lower is better. The order of the variants is the ranking order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Alias,
    Exact,
    Prefix,
    Fuzzy,
}

#[derive(Clone, Debug)]
pub struct Ranked {
    /// Index into the catalog slice passed to `rank`.
    pub index: usize,
    pub tier: Tier,
    /// 0 = title prefix, 1 = word prefix, 2 = acronym; 0 outside the prefix tier.
    pub sub: u8,
    pub score: u32,
    pub weight: f64,
}

pub struct Ranker {
    matcher: Matcher,
    buf: Vec<char>,
}

impl Default for Ranker {
    fn default() -> Self {
        Self { matcher: Matcher::new(Config::DEFAULT), buf: Vec::new() }
    }
}

impl Ranker {
    fn classify(&mut self, q: &str, pattern: &Pattern, item: &Item) -> Option<(Tier, u8, u32)> {
        if item.folded_aliases.iter().any(|a| a == q) {
            return Some((Tier::Alias, 0, 0));
        }
        if item.folded == q {
            return Some((Tier::Exact, 0, 0));
        }
        if item.folded.starts_with(q) {
            return Some((Tier::Prefix, 0, 0));
        }
        if item.words.iter().skip(1).any(|w| w.starts_with(q)) {
            return Some((Tier::Prefix, 1, 0));
        }
        if q.chars().count() >= 2 && item.acronym.starts_with(q) {
            return Some((Tier::Prefix, 2, 0));
        }
        if item.folded_aliases.iter().any(|a| a.starts_with(q)) {
            return Some((Tier::Prefix, 1, 0));
        }
        // One or two letters match almost anything as a subsequence.
        if q.chars().count() < 3 {
            return None;
        }
        let score = pattern.score(Utf32Str::new(&item.folded, &mut self.buf), &mut self.matcher)?;
        Some((Tier::Fuzzy, 0, score))
    }

    /// Rank `items` for `query`. `now` is seconds since the Unix epoch.
    /// An empty query returns the items that have any usage, most used first (FR-10a).
    pub fn rank(&mut self, query: &str, items: &[Item], usage: &Usage, now: f64, limit: usize) -> Vec<Ranked> {
        let q = fold(query);
        let mut out: Vec<Ranked> = Vec::new();

        if q.is_empty() {
            for (index, item) in items.iter().enumerate() {
                let weight = usage.weight("", &item.id, now);
                if weight > 0.0 {
                    out.push(Ranked { index, tier: Tier::Exact, sub: 0, score: 0, weight });
                }
            }
        } else {
            let pattern = Pattern::new(&q, CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy);
            for (index, item) in items.iter().enumerate() {
                if let Some((tier, sub, score)) = self.classify(&q, &pattern, item) {
                    let weight = usage.weight(&q, &item.id, now);
                    out.push(Ranked { index, tier, sub, score, weight });
                }
            }
        }

        out.sort_by(|a, b| {
            a.tier
                .cmp(&b.tier)
                .then(b.weight.total_cmp(&a.weight))
                .then(a.sub.cmp(&b.sub))
                .then(b.score.cmp(&a.score))
                .then(items[a.index].title.len().cmp(&items[b.index].title.len()))
                .then(items[a.index].title.cmp(&items[b.index].title))
                .then(items[a.index].id.cmp(&items[b.index].id))
        });
        out.truncate(limit);
        out
    }
}

/// Inline completion (DS-3a): the rest of the title when the title itself
/// starts with what was typed, compared case- and diacritic-insensitively.
pub fn completion<'a>(query: &str, title: &'a str) -> Option<&'a str> {
    let q = fold(query);
    if q.is_empty() {
        return None;
    }
    let mut q_chars = q.chars();
    for (i, c) in title.char_indices() {
        let folded = fold(&c.to_string());
        if folded.is_empty() {
            continue; // a combining mark
        }
        for fc in folded.chars() {
            match q_chars.next() {
                Some(qc) if qc == fc => {}
                Some(_) => return None,
                None => return Some(&title[i..]),
            }
        }
    }
    // The whole title was consumed: complete only if the query was too.
    if q_chars.next().is_none() { Some("") } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Vec<Item> {
        [
            "Terminal", "TextEdit", "Telegram", "System Settings", "Safari", "Visual Studio Code",
            "Notes", "Font Book", "WhatsApp", "T3 Code (Beta)", "Activity Monitor", "App Store",
        ]
        .iter()
        .map(|t| {
            let aliases = if *t == "Visual Studio Code" { vec!["te".to_string()] } else { vec![] };
            Item::new(format!("/Applications/{t}.app"), *t, Kind::App, aliases)
        })
        .collect()
    }

    fn titles(q: &str, usage: &Usage) -> Vec<String> {
        let items = catalog();
        Ranker::default().rank(q, &items, usage, 0.0, 10).iter().map(|r| items[r.index].title.clone()).collect()
    }

    #[test]
    fn alias_beats_everything() {
        assert_eq!(titles("te", &Usage::default())[0], "Visual Studio Code");
    }

    #[test]
    fn exact_beats_prefix() {
        let mut items = catalog();
        items.push(Item::new("x", "Note", Kind::App, vec![]));
        let r = Ranker::default().rank("note", &items, &Usage::default(), 0.0, 10);
        assert_eq!(items[r[0].index].title, "Note");
        assert_eq!(r[0].tier, Tier::Exact);
    }

    #[test]
    fn prefix_beats_fuzzy_and_acronym_is_prefix_tier() {
        let t = titles("sa", &Usage::default());
        assert_eq!(t[0], "Safari");
        let t = titles("vsc", &Usage::default());
        assert_eq!(t[0], "Visual Studio Code");
        let t = titles("am", &Usage::default());
        assert_eq!(t[0], "Activity Monitor");
    }

    #[test]
    fn usage_reorders_inside_a_tier_only() {
        let mut usage = Usage::default();
        for _ in 0..50 {
            usage.record("tex", "/Applications/TextEdit.app", 0.0);
        }
        // "ter" is a prefix of Terminal only; TextEdit can match at best fuzzily.
        // Heavy TextEdit usage must not lift it over the prefix tier.
        let t = titles("ter", &usage);
        assert_eq!(t[0], "Terminal");
        // Inside the prefix tier ("te"), TextEdit's item history does lift it
        // over Terminal and Telegram, but not over the alias.
        let t = titles("te", &usage);
        assert_eq!(&t[..2], ["Visual Studio Code", "TextEdit"]);
    }

    #[test]
    fn fuzzy_finds_subsequences() {
        let t = titles("stgs", &Usage::default());
        assert_eq!(t[0], "System Settings");
    }

    #[test]
    fn empty_query_lists_used_apps_most_used_first() {
        let mut usage = Usage::default();
        usage.record("", "/Applications/Notes.app", 0.0);
        for _ in 0..3 {
            usage.record("sa", "/Applications/Safari.app", 0.0);
        }
        assert_eq!(titles("", &usage), ["Safari", "Notes"]);
    }

    #[test]
    fn completion_is_the_rest_of_the_title() {
        assert_eq!(completion("te", "Terminal"), Some("rminal"));
        assert_eq!(completion("CAF", "Café Noir"), Some("é Noir"));
        assert_eq!(completion("ed", "TextEdit"), None);
        assert_eq!(completion("terminal", "Terminal"), Some(""));
    }
}
