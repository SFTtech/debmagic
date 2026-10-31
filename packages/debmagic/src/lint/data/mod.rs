//! Lines of a vendored lintian `data/` file, pinned to lintian 2.139.0.
//!
//! Blank lines and `#` comments are absent. `@if-vendor-is-not` lines stay;
//! a caller that treats Debian as the vendor drops those itself.

/// Non-empty, non-comment lines, trimmed.
pub(crate) fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

/// `key = value` or `key=value`.
pub(crate) fn assignment(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    let value = value.trim();
    (!key.is_empty()).then_some((key, value))
}
