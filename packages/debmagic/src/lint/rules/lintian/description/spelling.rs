//! The spelling passes of lintian's `Lintian::Spelling`, for `Description`
//! text: `check_spelling` over the vendored `spelling/corrections` and
//! `spelling/corrections-multiword`, and `check_spelling_picky` over
//! `spelling/corrections-case`.
//!
//! Difference from lintian: the spelling exceptions of the lintian group are
//! not applied (debmagic has no group), and the multi-word pattern with a
//! negative lookahead (`these package(?!...)`) is not loaded, because the
//! `regex` crate has no lookaround.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::data;

type Pairs = HashMap<&'static str, &'static str>;

fn pairs(text: &'static str) -> Pairs {
    data::lines(text)
        .filter_map(|line| line.split_once("||"))
        .collect()
}

static CORRECTIONS: LazyLock<Pairs> =
    LazyLock::new(|| pairs(include_str!("../../../data/spelling/corrections")));

static CORRECTIONS_CASE: LazyLock<Pairs> =
    LazyLock::new(|| pairs(include_str!("../../../data/spelling/corrections-case")));

static MULTIWORD: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/spelling/corrections-multiword"))
        .filter_map(|line| {
            let (pattern, correction) = line.split_once("||")?;
            let regex = Regex::new(&format!(r"\b({pattern})\b")).ok()?;
            Some((regex, correction))
        })
        .collect()
});

static META_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"meta\s+package").expect("valid regex"));

static BRACKETED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\[.+?\]").expect("valid regex"));

static HYPHEN_BREAK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\w-)\s*\n\s*").expect("valid regex"));

/// `(misspelled, correction)` pairs from `check_spelling_picky`, in the order
/// lintian finds them. A word is reported once. The `meta package` check
/// comes first and is not deduplicated.
pub(super) fn picky_capitalization(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    if META_PACKAGE.is_match(text) {
        found.push(("meta package".to_string(), "metapackage".to_string()));
    }
    // Text in square brackets may be a package list, so it is not checked.
    let stripped = BRACKETED.replace_all(text, "");
    let mut seen = HashSet::new();
    for word in stripped.split_whitespace() {
        let word = strip_word_edges(word);
        if let Some(correction) = CORRECTIONS_CASE.get(word)
            && seen.insert(word)
        {
            found.push((word.to_string(), (*correction).to_string()));
        }
    }
    found
}

/// `(misspelled, correction)` pairs from `check_spelling` over `text`, in the
/// order lintian reports them. A duplicated word is reported as
/// `"<word> <word> (duplicate word)"` with the word as correction.
pub(super) fn check_spelling(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let normalized = normalize(text);
    let mut seen: HashSet<String> = HashSet::new();
    let mut duplicates: HashSet<String> = HashSet::new();
    let mut last_word: Option<String> = None;
    let mut quoted = false;

    for raw in normalized.split_whitespace() {
        let ends_with_punct = raw.ends_with(TRAILING_PUNCTUATION);
        let word = raw.trim_end_matches(TRAILING_PUNCTUATION);

        if word.matches('"').count() % 2 == 1 {
            quoted = !quoted;
        }
        if last_word.as_deref() == Some(word)
            && !quoted
            && duplicates.insert(word.to_string())
            && !ends_with_punct
            && !repeated_in_parens(text, word)
        {
            found.push((format!("{word} {word} (duplicate word)"), word.to_string()));
        }
        last_word = (is_ascii_alpha(word) && !ends_with_punct).then(|| word.to_string());

        let lcword = word.to_lowercase();
        let Some(correction) = CORRECTIONS.get(lcword.as_str()) else {
            continue;
        };
        if is_one_to_five_capitals(word) {
            continue;
        }
        let correction = adjust_case(word, correction);
        if seen.insert(lcword) {
            found.push((word.to_string(), correction));
        }
    }

    for (regex, correction) in MULTIWORD.iter() {
        let Some(matched) = regex.captures(&normalized).and_then(|caps| caps.get(1)) else {
            continue;
        };
        let matched = matched.as_str();
        let correction = adjust_case(matched, correction);
        if seen.insert(matched.to_lowercase()) {
            found.push((format!("\"{matched}\""), format!("\"{correction}\"")));
        }
    }
    found
}

const TRAILING_PUNCTUATION: &[char] = &['.', ',', ';', ':', '?', '!'];

/// Lintian's text preparation before the word loop: brackets are dropped,
/// parentheses are dropped except `(s)`, a hyphen at a line break is joined,
/// and whitespace runs become single spaces.
fn normalize(text: &str) -> String {
    let no_brackets: String = text.chars().filter(|c| !matches!(c, '[' | ']')).collect();
    let no_parens = strip_parentheses(&no_brackets);
    let joined = HYPHEN_BREAK.replacen(&no_parens, 1, "$1");
    joined.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `s/(\((?!s\))|(?<!\(s)\))//gi`: `(` is dropped unless `s)` follows it, and
/// `)` is dropped unless `(s` precedes it.
fn strip_parentheses(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (index, &c) in chars.iter().enumerate() {
        let keep = match c {
            '(' => {
                chars
                    .get(index + 1)
                    .is_some_and(|n| n.eq_ignore_ascii_case(&'s'))
                    && chars.get(index + 2) == Some(&')')
            }
            ')' => {
                index >= 2 && chars[index - 2] == '(' && chars[index - 1].eq_ignore_ascii_case(&'s')
            }
            _ => true,
        };
        if keep {
            out.push(c);
        }
    }
    out
}

/// `\b<word>\s*\(<word>\b` or `\b<word>\)\s*<word>\b` in the original text:
/// the word is shown in parentheses, which is not a duplicate.
fn repeated_in_parens(original: &str, word: &str) -> bool {
    let word = regex::escape(word);
    [
        format!(r"\b{word}\s*\({word}\b"),
        format!(r"\b{word}\)\s*{word}\b"),
    ]
    .iter()
    .any(|pattern| Regex::new(pattern).is_ok_and(|regex| regex.is_match(original)))
}

/// `^[A-Z]+$` gives all capitals, `^[A-Z]` the first one.
fn adjust_case(word: &str, correction: &str) -> String {
    if !word.is_empty() && word.chars().all(|c| c.is_ascii_uppercase()) {
        correction.to_uppercase()
    } else if word.starts_with(|c: char| c.is_ascii_uppercase()) {
        let mut chars = correction.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().chain(chars).collect(),
            None => String::new(),
        }
    } else {
        correction.to_string()
    }
}

fn is_ascii_alpha(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_ascii_alphabetic())
}

/// `^[A-Z]{1,5}\z`: an acronym is not a misspelling.
fn is_one_to_five_capitals(word: &str) -> bool {
    (1..=5).contains(&word.len()) && word.chars().all(|c| c.is_ascii_uppercase())
}

/// Strips a leading `(` and trailing `).,?!:;` characters, as the picky check does.
fn strip_word_edges(word: &str) -> &str {
    let word = word.strip_prefix('(').unwrap_or(word);
    word.trim_end_matches([')', '.', ',', '?', '!', ':', ';'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrects_case_once_per_word() {
        assert_eq!(
            picky_capitalization("debian is debian, (debian)."),
            [("debian".to_string(), "Debian".to_string())]
        );
    }

    #[test]
    fn skips_bracketed_text_and_reports_meta_package() {
        assert_eq!(
            picky_capitalization("[debian] a meta package"),
            [("meta package".to_string(), "metapackage".to_string())]
        );
    }

    #[test]
    fn spells_and_reports_duplicates() {
        assert_eq!(
            check_spelling("a mroe, mroe tool"),
            [("mroe".to_string(), "more".to_string()),]
        );
        assert_eq!(
            check_spelling("these these tools"),
            [(
                "these these (duplicate word)".to_string(),
                "these".to_string()
            )]
        );
    }

    #[test]
    fn keeps_acronyms_and_plural_s() {
        assert!(check_spelling("the USA (s) tool").is_empty());
    }
}
