//! The source format of a Source package or Source tree, read once for every
//! Rule, and lintian's format-based nativeness (`Processable/Source/Format.pm`).
//!
//! A Source package declares its format in the Dsc `Format` field. A Source
//! tree has no Dsc; it is read the way `dpkg-source -b` would read it, from the
//! first line of `debian/source/format`. Rules differ in what a missing format
//! means, so [`field`] returns `None` for it and each caller maps that itself.

use std::sync::LazyLock;

use regex::Regex;

use super::context::SourceContext;

static TWO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*2\.0\s*$").expect("valid regex"));
/// Perl `/^\s*3\.0\s+\(quilt|git\)\s*$/` alternates at `|`, not inside the
/// parentheses: "starts with `3.0 (quilt`" OR "ends with `git)`". Kept as
/// lintian has it.
static QUILT_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*3\.0\s+\(quilt").expect("valid regex"));
static GIT_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"git\)\s*$").expect("valid regex"));
static NATIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*3\.0\s+\(native\)\s*$").expect("valid regex"));

/// The declared format. On a Source package: the Dsc `Format` unfolded, when
/// declared, even if empty. On a Source tree: the trimmed first line of
/// `debian/source/format` when it is a regular file or hardlink with UTF-8
/// text and that line is not empty. `None` otherwise.
pub(crate) fn field(ctx: &mut SourceContext<'_>) -> Option<String> {
    if let Some(fields) = ctx.dsc().map(|dsc| dsc.fields()) {
        return fields.unfolded_value("Format");
    }
    let bytes = ctx.files()?.read_file("debian/source/format")?;
    let text = String::from_utf8(bytes).ok()?;
    let first = text.lines().next()?.trim();
    (!first.is_empty()).then(|| first.to_string())
}

/// lintian's `source_format`: the declared format, `1.0` when it is missing
/// or empty (`value('Format') || '1.0'`).
pub(crate) fn lintian_format(ctx: &mut SourceContext<'_>) -> String {
    field(ctx)
        .filter(|format| !format.is_empty())
        .unwrap_or_else(|| "1.0".to_string())
}

/// What lintian's `native` concludes from the format alone: `Some(false)` for
/// `2.0` and the `3.0 (quilt` / `git)` patterns, `Some(true)` for
/// `3.0 (native)`, `None` when the package must be judged by its `.diff.gz`.
pub(crate) fn nativeness(format: &str) -> Option<bool> {
    if TWO.is_match(format) || QUILT_PREFIX.is_match(format) || GIT_SUFFIX.is_match(format) {
        return Some(false);
    }
    if NATIVE.is_match(format) {
        return Some(true);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nativeness_follows_format_pm_including_its_alternation() {
        assert_eq!(nativeness("3.0 (native)"), Some(true));
        assert_eq!(nativeness(" 3.0  (native) "), Some(true));
        assert_eq!(nativeness("3.0 (quilt)"), Some(false));
        assert_eq!(nativeness("3.0 (git)"), Some(false));
        assert_eq!(nativeness("2.0"), Some(false));
        // Perl reads `\(quilt|git\)` as two alternatives.
        assert_eq!(nativeness("3.0 (quilt) junk"), Some(false));
        assert_eq!(nativeness("anything git)"), Some(false));
        assert_eq!(nativeness("1.0"), None);
        assert_eq!(nativeness("3.0 (custom)"), None);
    }
}
