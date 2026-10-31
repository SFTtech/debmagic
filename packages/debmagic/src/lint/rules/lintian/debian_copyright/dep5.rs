//! The first line of a DEP-5 `License` field, as `Dep5.pm` splits it.

use std::sync::LazyLock;

use regex::Regex;

use crate::lint::debian_copyright::CopyrightParagraph;

static AND_OR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+(?:and|or)\s+").expect("valid regex"));

/// Lowercased short license: the first line, with `(),` turned into spaces.
pub(super) fn license_identifier(value: &str) -> String {
    let first = value.split('\n').next().unwrap_or("");
    first
        .replace([',', '(', ')'], " ")
        .trim()
        .to_ascii_lowercase()
}

/// License body after the short-name line, with both ends trimmed.
pub(super) fn license_text(value: &str) -> String {
    value
        .split_once('\n')
        .map(|(_, rest)| rest.trim().to_string())
        .unwrap_or_default()
}

/// Short names split on `and` / `or`. Empty pieces are dropped.
pub(super) fn license_names(identifier: &str) -> Vec<String> {
    AND_OR
        .split(identifier)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

/// Wildcards of a follower paragraph. The first follower that has `License`
/// and `Copyright` but no `Files` stands for `*`.
pub(super) fn follower_wildcards(paragraph: &CopyrightParagraph, index: usize) -> Vec<String> {
    if index == 0
        && paragraph.declares("License")
        && paragraph.declares("Copyright")
        && !paragraph.declares("Files")
    {
        vec!["*".to_string()]
    } else {
        paragraph
            .trimmed_list("Files")
            .into_iter()
            .map(str::to_string)
            .collect()
    }
}
