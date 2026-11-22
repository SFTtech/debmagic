//! The picky capitalization pass of lintian's `Lintian::Spelling`
//! (`check_spelling_picky`), over the vendored `spelling/corrections-case`.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::data;

static CORRECTIONS_CASE: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/spelling/corrections-case"))
        .filter_map(|line| line.split_once("||"))
        .collect()
});

static META_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"meta\s+package").expect("valid regex"));

static BRACKETED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\[.+?\]").expect("valid regex"));

/// The `(misspelled, correction)` pairs lintian reports for `text`, in the
/// order it finds them. A word is reported once. The `meta package` check
/// comes first, and it is not deduplicated against the words.
pub(super) fn picky_capitalization(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    if META_PACKAGE.is_match(text) {
        found.push(("meta package".to_string(), "metapackage".to_string()));
    }
    // Text in square brackets may be a package list, so it is not checked.
    let stripped = BRACKETED.replace_all(text, "");
    let mut seen = HashSet::new();
    for word in stripped.split_whitespace() {
        let word = word.strip_prefix('(').unwrap_or(word);
        let word = word.trim_end_matches([')', '.', ',', '?', '!', ':', ';']);
        if let Some(correction) = CORRECTIONS_CASE.get(word)
            && seen.insert(word)
        {
            found.push((word.to_string(), (*correction).to_string()));
        }
    }
    found
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
}
