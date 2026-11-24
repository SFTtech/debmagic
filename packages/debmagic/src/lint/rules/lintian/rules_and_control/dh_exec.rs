//! dh-exec scripts among the debhelper config files of `debian/`.
//!
//! Lintian's `debhelper` Check walks the direct children of `debian/`, and
//! for a filename config (`install`, `links`, `<package>.mime`, ...) whose
//! `#!` line names `dh-exec`, reads its lines for the dh-exec features it
//! uses. This module is that walk, up to the per-file facts the Tags report.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::SourceContext;
use crate::lint::file_tree::EntryKind;

use super::rules_text::read_utf8;

/// lintian 2.139.0 `$USES_EXECUTABLE_FILES`: dh-exec scripts are honoured from
/// this compat level.
const USES_EXECUTABLE_FILES: u32 = 9;
/// lintian 2.139.0 `$DEBHELPER_DEB_SUBST_LEVEL`: dh_install substitutes the
/// `DEB_HOST_*` variables itself from this compat level.
const DEBHELPER_DEB_SUBST_LEVEL: u32 = 13;

/// lintian 2.139.0 `data/debhelper/filename-config-files`.
const FILENAME_CONFIGS: &[&str] = &[
    "dirs",
    "docs",
    "examples",
    "info",
    "install",
    "links",
    "manpages",
    "mime",
    "sgmlcatalogs",
    "sharedmimeinfo",
    "wm",
];

static SCRIPT_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:.*\.)?(?:post|pre)(?:inst|rm)$").expect("valid regex"));
static SUBSTITUTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{([^}]+)\}").expect("valid regex"));
static USR_LIB_ONE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/\$\{([^}]+)\}/?$").expect("valid regex"));
static USR_LIB_TWO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib/\$\{([^}]+)\}/?\s+/usr/lib/\$\{([^}]+)\}/?$").expect("valid regex")
});
static USR_LIB_OTHER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/\$\{([^}]+)\}\S+$").expect("valid regex"));
/// The variables dh-exec substitutes without the package exporting them.
static KNOWN_SUBSTITUTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^DEB_(?:BUILD|HOST)_(?:ARCH(?:_OS|_CPU|_BITS|_ENDIAN)?|GNU_(?:CPU|SYSTEM|TYPE)|MULTIARCH)$",
    )
    .expect("valid regex")
});
static INSTALL_ARROW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[ \t]=>[ \t]").expect("valid regex"));
static BRACKET_FILTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]+\]").expect("valid regex"));
static ANGLE_FILTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>").expect("valid regex"));

/// A dh-exec script and the features its lines use.
pub(super) struct DhExecScript {
    pub(super) path: PathBuf,
    /// Text after the last dot of the name, or the whole name.
    pub(super) category: String,
    pub(super) substitution: bool,
    /// The first `${...}` of each line that is not a `DEB_BUILD_*` or
    /// `DEB_HOST_*` variable dh-exec knows: the 1-based line and the name.
    pub(super) unknown_variables: Vec<(u32, String)>,
    pub(super) install: bool,
    pub(super) filter: bool,
    /// lintian's `$dhe_needed` after the last line: the script uses a
    /// dh-exec feature (a rename, a filter, or a `usr/lib` variable that is
    /// not a plain wildcard).
    pub(super) needed: bool,
}

/// Every dh-exec script, in path order. Empty when `debian/rules` shows no dh
/// command, or the declared compat level is below 9.
pub(super) fn dh_exec_scripts(ctx: &mut SourceContext<'_>) -> Vec<DhExecScript> {
    let Some(facts) = ctx.debhelper() else {
        return Vec::new();
    };
    if !facts.uses_debhelper() || facts.level() < USES_EXECUTABLE_FILES {
        return Vec::new();
    }
    let level = facts.level();
    let Some(files) = ctx.files() else {
        return Vec::new();
    };
    let mut scripts = Vec::new();
    for entry in files.entries() {
        let path = entry.path();
        if path.parent() != Some(Path::new("debian")) {
            continue;
        }
        if !matches!(
            entry.kind(),
            EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
        ) {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name == "rules" || SCRIPT_NAME.is_match(name) {
            continue;
        }
        let category = category(name);
        if category.is_empty() || !FILENAME_CONFIGS.contains(&category) {
            continue;
        }
        let Some(text) = read_utf8(files, &path.to_string_lossy()) else {
            continue;
        };
        if !hashbang(&text).contains("dh-exec") {
            continue;
        }
        let mut script = DhExecScript {
            path: path.to_path_buf(),
            category: category.to_string(),
            substitution: false,
            unknown_variables: Vec::new(),
            install: false,
            filter: false,
            needed: false,
        };
        // Perl's `<$fd>` reads no empty line after the final newline.
        let body = text.strip_suffix('\n').unwrap_or(&text);
        for (index, line) in body.split('\n').enumerate() {
            if let Some(captures) = SUBSTITUTION.captures(line) {
                script.substitution = level < DEBHELPER_DEB_SUBST_LEVEL;
                let name = &captures[1];
                if !KNOWN_SUBSTITUTION.is_match(name) {
                    script
                        .unknown_variables
                        .push((index as u32 + 1, name.to_string()));
                }
            }
            if INSTALL_ARROW.is_match(line) {
                script.install = true;
            }
            if BRACKET_FILTER.is_match(line) || ANGLE_FILTER.is_match(line) {
                script.filter = true;
            }
            // lintian's `$dhe_needed`: reset on every line to the features seen
            // so far, then set again by a `usr/lib/${...}` line that is not a
            // plain wildcard (compat below 13 only).
            script.needed = script.install || script.filter;
            if level < DEBHELPER_DEB_SUBST_LEVEL
                && let Some((sv, dv)) = usr_lib_substitution(line)
                && KNOWN_SUBSTITUTION.is_match(sv)
            {
                script.needed |= !dv.is_none_or(|dv| sv == dv);
            }
        }
        scripts.push(script);
    }
    scripts
}

/// The variables of a `usr/lib/${sv}` line (first variable, and the second
/// one of `usr/lib/${sv} /usr/lib/${dv}` when there is one), as lintian's
/// useless-usage check reads them.
fn usr_lib_substitution(line: &str) -> Option<(&str, Option<&str>)> {
    if let Some(captures) = USR_LIB_ONE.captures(line) {
        return Some((captures.get(1)?.as_str(), None));
    }
    if let Some(captures) = USR_LIB_TWO.captures(line) {
        return Some((
            captures.get(1)?.as_str(),
            captures.get(2).map(|dv| dv.as_str()),
        ));
    }
    let captures = USR_LIB_OTHER.captures(line)?;
    Some((captures.get(1)?.as_str(), None))
}

/// Perl `s/^.+\.//`: the text after the last dot when something precedes it.
fn category(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((prefix, suffix)) if !prefix.is_empty() => suffix,
        _ => name,
    }
}

/// lintian's `hashbang`: the first line after `#!`, trimmed. Empty without a
/// `#!` start or when nothing follows the two bytes.
fn hashbang(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("#!") else {
        return "";
    };
    if rest.is_empty() {
        return "";
    }
    rest.split('\n').next().unwrap_or("").trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_is_the_last_suffix() {
        assert_eq!(category("install"), "install");
        assert_eq!(category("pkg.install"), "install");
        assert_eq!(category("pkg.extra.links"), "links");
        assert_eq!(category(".install"), ".install");
        assert_eq!(category("pkg."), "");
    }

    #[test]
    fn hashbang_trims_the_first_line() {
        assert_eq!(hashbang("#! /usr/bin/dh-exec \nfoo\n"), "/usr/bin/dh-exec");
        assert_eq!(hashbang("usr/bin/foo\n"), "");
        assert_eq!(hashbang("#!"), "");
    }
}
