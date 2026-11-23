//! Lintian's derivative tables: `data/common/derivative-changed-by`,
//! `data/fields/derivative-fields` and `data/fields/derivative-versions`,
//! vendored from lintian 2.144.0.
//!
//! Lintian reads a table from `vendors/<vendor>/` before the base `data/`
//! directory. Debian has no vendor directory, so the base tables are the ones a
//! lint run checks against: debmagic models Debian as the vendor and has no way
//! to select another one. The pinned clone's `vendors/pureos/main/` overrides
//! belong to another vendor and are not vendored. None of these tables has
//! `@if-vendor-is-not` lines.
//!
//! Each entry is `regex ~~ explanation`, or `field ~~ regex ~~ explanation` for
//! the field table. Lintian compiles the regex as a Perl pattern; this module
//! compiles it with the `regex` crate, which agrees on the patterns the tables
//! use (`.` only).

use std::sync::LazyLock;

use regex::Regex;

use super::context::SourceContext;
use super::data;
use super::debian_control::ControlParagraph;

/// A `regex ~~ explanation` entry: lintian's `qr/regex/` and its explanation.
pub(crate) struct Pattern {
    pub(crate) regex: Regex,
    pub(crate) explanation: String,
}

/// A `field ~~ regex ~~ explanation` entry of the field table.
pub(crate) struct FieldPattern {
    pub(crate) field: String,
    pub(crate) pattern: Pattern,
}

/// Lintian's `data/common/derivative-changed-by`, in file order.
pub(crate) fn changed_by() -> &'static [Pattern] {
    static TABLE: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
        data::lines(include_str!("data/common/derivative-changed-by"))
            .map(pattern)
            .collect()
    });
    &TABLE
}

/// Lintian's `data/fields/derivative-fields`, in file order.
pub(crate) fn fields() -> &'static [FieldPattern] {
    static TABLE: LazyLock<Vec<FieldPattern>> = LazyLock::new(|| {
        data::lines(include_str!("data/fields/derivative-fields"))
            .map(|line| {
                let (field, rest) = line.split_once("~~").expect("vendored entry has a `~~`");
                FieldPattern {
                    field: field.trim().to_string(),
                    pattern: pattern(rest),
                }
            })
            .collect()
    });
    &TABLE
}

/// Lintian's `data/fields/derivative-versions`, in file order.
pub(crate) fn versions() -> &'static [Pattern] {
    static TABLE: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
        data::lines(include_str!("data/fields/derivative-versions"))
            .map(pattern)
            .collect()
    });
    &TABLE
}

/// The paragraph lintian's `processable->fields` is for a Source subject: the
/// Dsc's fields, or on a Source tree the source paragraph of `debian/control`.
pub(crate) fn source_fields<'a>(ctx: &'a SourceContext<'_>) -> Option<&'a ControlParagraph> {
    match ctx.dsc() {
        Some(dsc) => Some(dsc.fields()),
        None => ctx.debian_control().map(|control| control.source()),
    }
}

/// `regex ~~ explanation`. Lintian splits at the first `~~` only.
fn pattern(text: &str) -> Pattern {
    let (regex, explanation) = text.split_once("~~").expect("vendored entry has a `~~`");
    Pattern {
        regex: Regex::new(regex.trim()).expect("vendored derivative regex is valid"),
        explanation: explanation.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendored_tables_are_the_base_debian_ones() {
        let changed_by = changed_by();
        assert_eq!(changed_by.len(), 1);
        assert_eq!(changed_by[0].regex.as_str(), ".");
        assert_eq!(changed_by[0].explanation, "always matches");

        let fields = fields();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field, "Source");
        assert_eq!(fields[0].pattern.regex.as_str(), ".");
        assert_eq!(fields[0].pattern.explanation, "always matches");

        let versions = versions();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].regex.as_str(), ".");
        assert_eq!(versions[0].explanation, "always matches");
    }
}
