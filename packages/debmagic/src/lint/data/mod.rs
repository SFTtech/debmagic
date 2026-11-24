//! Lines of a vendored lintian `data/` file, copied from the lintian clone that the parity suite pins (2.144.0).
//!
//! Blank lines and `#` comments are absent. `@if-vendor-is-not` lines stay;
//! a caller that treats Debian as the vendor drops those itself.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::OnceLock;

/// The `--include-dir` of the run, if one was given.
static INCLUDE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Records the hidden `--include-dir` of `debmagic lint`. The first call wins;
/// it runs before any Rule, so every table reads the same directory.
pub fn set_include_dir(dir: PathBuf) {
    let _ = INCLUDE_DIR.set(dir);
}

/// The text of the data file `name` (such as `debhelper/compat-level`): the
/// copy under `vendors/<vendor>/<profile>/data/` of the `--include-dir`, when
/// the run has one and it names that file, else `vendored`.
///
/// As in lintian, an override that starts with `@include-parent` keeps the
/// vendored lines and its own lines follow, so a later line wins; without it
/// the override replaces the table. Only the tables that a Rule reads through
/// this function can be overridden, so the flag changes nothing else.
pub(crate) fn table(name: &str, vendored: &'static str) -> Cow<'static, str> {
    let Some(dir) = INCLUDE_DIR.get() else {
        return Cow::Borrowed(vendored);
    };
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir.join("vendors"))
        .into_iter()
        .flatten()
        .flatten()
        .flat_map(|vendor| {
            std::fs::read_dir(vendor.path())
                .into_iter()
                .flatten()
                .flatten()
        })
        .map(|profile| profile.path().join("data").join(name))
        .collect();
    paths.sort();
    match paths
        .into_iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
    {
        Some(text) => overlay(vendored, &text),
        None => Cow::Borrowed(vendored),
    }
}

/// An override file's text over the vendored one: the override replaces the
/// table, unless it has an `@include-parent` line, which puts the vendored
/// lines first and drops that line.
fn overlay(vendored: &'static str, text: &str) -> Cow<'static, str> {
    if !text.lines().any(|line| line.trim() == "@include-parent") {
        return Cow::Owned(text.to_string());
    }
    let own: Vec<&str> = text
        .lines()
        .filter(|line| line.trim() != "@include-parent")
        .collect();
    Cow::Owned(format!("{vendored}\n{}", own.join("\n")))
}

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

#[cfg(test)]
mod tests {
    use super::overlay;

    #[test]
    fn override_replaces_the_table_without_include_parent() {
        assert_eq!(overlay("a=1\n", "b=2\n"), "b=2\n");
    }

    #[test]
    fn include_parent_keeps_the_vendored_lines_first() {
        assert_eq!(
            overlay("a=1\nb=1\n", "@include-parent\nb=2\n"),
            "a=1\nb=1\n\nb=2"
        );
    }
}
