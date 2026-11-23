//! The source-paragraph Python version field from lintian 2.144.0
//! `Check::Languages::Python`.
//!
//! Only `X-Python3-Version` is read. The ancient threshold is lintian 2.144.0
//! `data/python/versions`.

use std::sync::LazyLock;

use regex::Regex;

use crate::lint::debian_control::DebianControl;

pub(super) const FIELDS: &[&str] = &["X-Python3-Version"];

/// The `ancient-python` version of `data/python/versions`.
static ANCIENT: LazyLock<Option<&'static str>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/python/versions")).find_map(|line| {
        let (key, value) = crate::lint::data::assignment(line)?;
        (key == "ancient-python").then_some(value)
    })
});

pub(super) fn ancient_threshold() -> Option<&'static str> {
    *ANCIENT
}

/// The `old-python` version of `data/python/versions`.
static OLD: LazyLock<Option<&'static str>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/python/versions")).find_map(|line| {
        let (key, value) = crate::lint::data::assignment(line)?;
        (key == "old-python").then_some(value)
    })
});

pub(super) fn old_threshold() -> Option<&'static str> {
    *OLD
}

static NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9]+\.[0-9]+$").expect("valid regex"));
static GE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^>=\s*[0-9]+\.[0-9]+$").expect("valid regex"));
static LT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^<<\s*[0-9]+\.[0-9]+$").expect("valid regex"));
static CURRENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^current$").expect("valid regex"));
static ALL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^all$").expect("valid regex"));
static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*").expect("valid regex"));
static ENCODES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^python[0-9]-").expect("valid regex"));

pub(super) fn field_values(control: &DebianControl) -> Vec<(&'static str, &str)> {
    FIELDS
        .iter()
        .filter_map(|field| control.source().get(field).map(|value| (*field, value)))
        .collect()
}

pub(super) fn current_is_deprecated(value: &str) -> bool {
    value.starts_with("current")
}

pub(super) fn is_malformed(value: &str) -> bool {
    let parts: Vec<&str> = COMMA.split(value).collect();
    if parts.len() > 2 {
        return parts.iter().any(|part| !NUMBER.is_match(part));
    }
    let rules: &[(&Regex, Option<&Regex>)] = &[
        (&NUMBER, Some(&NUMBER)),
        (&NUMBER, None),
        (&GE, Some(&LT)),
        (&GE, None),
        (&CURRENT, Some(&GE)),
        (&CURRENT, None),
        (&ALL, None),
    ];
    let first = parts.first().copied().unwrap_or("");
    let second = parts.get(1).copied();
    !rules.iter().any(|(first_rule, second_rule)| {
        first_rule.is_match(first)
            && match (second, second_rule) {
                (Some(part), Some(rule)) => rule.is_match(part),
                (None, None) => true,
                _ => false,
            }
    })
}

pub(super) fn encodes_python_version(name: &str) -> bool {
    ENCODES.is_match(name) && name != "python3-defaults"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_shapes() {
        assert!(is_malformed("current 2.7"));
        assert!(is_malformed("invalid"));
        assert!(is_malformed("2.7, foo, 3.5"));
        assert!(!is_malformed(">= 3.6"));
        assert!(!is_malformed("all"));
        assert!(!is_malformed("current"));
        assert!(!is_malformed("2.7"));
        assert!(!is_malformed("2.7, 2.6"));
        assert!(!is_malformed(">= 2.7, << 3.0"));
        assert!(!is_malformed("2.7, 2.6, 2.5"));
        assert!(!is_malformed("current, >= 2.7"));
    }

    #[test]
    fn package_names() {
        assert!(encodes_python_version("python3-foo"));
        assert!(encodes_python_version("python2-foo"));
        assert!(!encodes_python_version("python3-defaults"));
        assert!(!encodes_python_version("python-foo"));
        assert!(!encodes_python_version("example"));
    }
}
