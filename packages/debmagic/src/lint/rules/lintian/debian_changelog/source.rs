//! Nativeness and the source-changelog facts the epoch, reuse, and historical
//! Tags share. Version pieces stay in this module.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::SourceContext;
use crate::lint::debian_changelog::ChangelogEntry;
use crate::lint::diagnostic::Location;
use crate::lint::file_tree::EntryKind;

use super::version::{ParsedVersion, historical_no_epoch, parse_version, strip_epoch, versions_gt};

static EXPLICIT_TWO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*2\.0\s*$").expect("valid regex"));
/// Perl `/^\s*3\.0\s+\(quilt|git\)\s*$/` alternates at `|`, not inside the
/// `3.0` prefix: `3.0 (quilt` anywhere from the start, or a string ending in
/// `git)`.
static QUILT_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*3\.0\s+\(quilt").expect("valid regex"));
static GIT_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"git\)\s*$").expect("valid regex"));
static EXPLICIT_NATIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*3\.0\s+\(native\)\s*$").expect("valid regex"));
static EPOCH_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bepoch\b").expect("valid regex"));

pub(super) fn changelog_location(line: u32) -> Location {
    Location {
        path: PathBuf::from("debian/changelog"),
        line: Some(line),
        column: None,
    }
}

/// Lintian's source `native`: explicit `3.0 (native)`, not `2.0` / quilt / git,
/// else a 1.0-style package, native when the package has no `.diff.gz`.
///
/// A Source package with a Dsc is native when `Files` has no
/// `{name}_{version}.diff.gz`, exactly as lintian reads it.
///
/// A Source tree has no Dsc, so it is modelled as the package `dpkg-source -b`
/// would build from it, seen the way lintian sees that package: a 1.0 tree is
/// non-native (a `.diff.gz` exists) only when `dpkg-source` finds an upstream
/// beside the tree, an `{name}_{upstream}.orig.tar.{gz,bz2,xz,lzma}` file or an
/// `{name}-{upstream}.orig` directory in the tree's parent directory, where
/// `{upstream}` is the epochless Version without its last `-revision`. Without
/// one (and always when the Version has no revision) `dpkg-source` packs the
/// whole tree as a native tarball, with or without a hyphen in the Version, so
/// `1.0-1` is native: `cruft-native-dash-version` expects lintian to report
/// `1.0-1 (for native)`. The Version's hyphen alone says nothing about the
/// diff. Source packages never use this branch.
pub(in crate::lint::rules::lintian) fn package_is_native(ctx: &mut SourceContext<'_>) -> bool {
    let format = source_format(ctx);
    let version = ctx
        .debian_changelog()
        .and_then(|changelog| changelog.entries().first())
        .map(|entry| entry.version().to_string())
        .unwrap_or_default();
    let diff_present = diff_gz_present(ctx, &version);
    if explicit_non_native(&format) {
        return false;
    }
    if EXPLICIT_NATIVE.is_match(&format) {
        return true;
    }
    if version.is_empty() {
        return false;
    }
    match diff_present {
        Some(present) => !present,
        None => !source_tree_has_orig(ctx, &version),
    }
}

fn explicit_non_native(format: &str) -> bool {
    EXPLICIT_TWO.is_match(format) || QUILT_PREFIX.is_match(format) || GIT_SUFFIX.is_match(format)
}

fn source_format(ctx: &mut SourceContext<'_>) -> String {
    if let Some(format) = ctx.dsc().and_then(|dsc| dsc.fields().get("Format")) {
        return format.trim().to_string();
    }
    let Some(files) = ctx.files() else {
        return "1.0".to_string();
    };
    let Some(entry) = files.get(Path::new("debian/source/format")) else {
        return "1.0".to_string();
    };
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return "1.0".to_string();
    }
    let Ok(bytes) = files.read(entry) else {
        return "1.0".to_string();
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return "1.0".to_string();
    };
    text.lines().next().unwrap_or("1.0").trim().to_string()
}

fn diff_gz_present(ctx: &SourceContext<'_>, version: &str) -> Option<bool> {
    let dsc = ctx.dsc()?;
    if version.is_empty() {
        return Some(false);
    }
    let name = dsc
        .fields()
        .get("Source")
        .or_else(|| {
            ctx.debian_control()
                .and_then(|control| control.source().get("Source"))
        })
        .unwrap_or("")
        .trim();
    let diffname = format!("{name}_{}.diff.gz", strip_epoch(version));
    let files = dsc.fields().get("Files").unwrap_or("");
    Some(files.split_whitespace().any(|part| part == diffname))
}

/// Whether `dpkg-source -b` would find an upstream beside a Source tree and so
/// build a `.diff.gz`. See [`package_is_native`].
fn source_tree_has_orig(ctx: &SourceContext<'_>, version: &str) -> bool {
    let Some((upstream, _revision)) = strip_epoch(version).rsplit_once('-') else {
        return false;
    };
    let name = ctx
        .debian_changelog()
        .and_then(|changelog| changelog.entries().first())
        .map(|entry| entry.source().to_string())
        .unwrap_or_default();
    let Some(parent) = ctx.path().parent() else {
        return false;
    };
    if name.is_empty() {
        return false;
    }
    let tarball = |extension: &str| parent.join(format!("{name}_{upstream}.orig.tar.{extension}"));
    ["gz", "bz2", "xz", "lzma"]
        .into_iter()
        .any(|extension| tarball(extension).is_file())
        || parent.join(format!("{name}-{upstream}.orig")).is_dir()
}

pub(super) struct EpochDelta {
    pub line: u32,
    pub previous: String,
    pub latest: String,
    pub native: bool,
    pub upstream_went_backwards: bool,
    pub mentions_epoch: bool,
}

/// Epoch differs between the newest entry and the one before it. `None` when
/// either version does not parse for the package's nativeness.
pub(super) fn epoch_delta(ctx: &mut SourceContext<'_>) -> Option<EpochDelta> {
    let native = package_is_native(ctx);
    let changelog = ctx.debian_changelog()?;
    let [latest, previous, ..] = changelog.entries() else {
        return None;
    };
    let latest_parsed = parse_version(latest.version(), native)?;
    let previous_parsed = parse_version(previous.version(), native)?;
    if latest_parsed.epoch == previous_parsed.epoch {
        return None;
    }
    Some(EpochDelta {
        line: latest.line(),
        previous: previous_parsed.literal,
        latest: latest_parsed.literal,
        native,
        upstream_went_backwards: versions_gt(&previous_parsed.upstream, &latest_parsed.upstream),
        mentions_epoch: EPOCH_WORD.is_match(latest.changes()),
    })
}

pub(super) fn epoch_arrow(delta: &EpochDelta) -> String {
    format!("{} -> {}", delta.previous, delta.latest)
}

/// The second entry's version does not parse for the current nativeness.
pub(super) fn odd_historical(ctx: &mut SourceContext<'_>) -> Option<(u32, String)> {
    let native = package_is_native(ctx);
    let changelog = ctx.debian_changelog()?;
    let [_, previous, ..] = changelog.entries() else {
        return None;
    };
    if parse_version(previous.version(), native).is_some() {
        return None;
    }
    let kind = if native { "native" } else { "non-native" };
    Some((
        previous.line(),
        format!("{} (for {kind})", previous.version()),
    ))
}

/// Newest `no_epoch` equals an older entry with the epoch regex-stripped, same
/// source name. Stops at the first such entry.
pub(super) fn reused_version(ctx: &mut SourceContext<'_>) -> Option<(u32, String)> {
    let native = package_is_native(ctx);
    let changelog = ctx.debian_changelog()?;
    let entries = changelog.entries();
    let [latest, rest @ ..] = entries else {
        return None;
    };
    if rest.is_empty() {
        return None;
    }
    let latest_parsed = parse_version(latest.version(), native)?;
    for entry in rest {
        let Some(no_epoch) = historical_no_epoch(entry.version()) else {
            continue;
        };
        if latest_parsed.no_epoch == no_epoch && latest.source() == entry.source() {
            return Some((latest.line(), reuse_extra(&latest_parsed, entry)));
        }
    }
    None
}

fn reuse_extra(latest: &ParsedVersion, entry: &ChangelogEntry) -> String {
    format!(
        "{} ~ {} (last used: {})",
        latest.literal,
        entry.version(),
        entry.date()
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::lint::context::SourceTreeData;

    use super::super::ln0870_malformed_debian_changelog_version::MalformedDebianChangelogVersion;
    use super::*;

    /// `<tmp>/src` with a changelog for `version` and, when `format` is given,
    /// `debian/source/format`; returns the temp dir so siblings can be added.
    fn tree(format: Option<&str>, version: &str) -> tempfile::TempDir {
        let work = tempfile::tempdir().expect("temp dir");
        let debian = work.path().join("src/debian");
        fs::create_dir_all(debian.join("source")).expect("debian/source");
        fs::write(
            debian.join("changelog"),
            format!(
                "example ({version}) unstable; urgency=low\n\n  * x\n\n -- A <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
            ),
        )
        .expect("changelog");
        if let Some(format) = format {
            fs::write(debian.join("source/format"), format).expect("format");
        }
        work
    }

    fn native(work: &tempfile::TempDir) -> bool {
        let data = SourceTreeData::new(work.path().join("src"));
        let mut ctx = SourceContext::bind_tree(&data, &MalformedDebianChangelogVersion);
        package_is_native(&mut ctx)
    }

    #[test]
    fn explicit_formats_ignore_the_orig() {
        assert!(native(&tree(Some("3.0 (native)\n"), "1.0-1")));
        assert!(!native(&tree(Some("3.0 (quilt)\n"), "1.0-1")));
        assert!(!native(&tree(Some("3.0 (quilt)\n"), "1.0")));
    }

    #[test]
    fn format_1_tree_without_orig_is_native_even_with_a_revision() {
        assert!(native(&tree(Some("1.0\n"), "1.0-1")));
        assert!(native(&tree(None, "2:1.0-1")));
        assert!(native(&tree(None, "1.0")));
    }

    #[test]
    fn format_1_tree_with_orig_beside_it_is_not_native() {
        let work = tree(Some("1.0\n"), "1:1.0-1");
        fs::write(work.path().join("example_1.0.orig.tar.xz"), b"").expect("orig");
        assert!(!native(&work));

        let work = tree(None, "1.0-2");
        fs::create_dir(work.path().join("example-1.0.orig")).expect("orig dir");
        assert!(!native(&work));
    }

    #[test]
    fn orig_beside_the_tree_needs_a_revision_and_the_upstream_version() {
        let work = tree(None, "1.0");
        fs::write(work.path().join("example_1.0.orig.tar.gz"), b"").expect("orig");
        assert!(native(&work));

        let work = tree(None, "1.0-1");
        fs::write(work.path().join("example_2.0.orig.tar.gz"), b"").expect("other orig");
        assert!(native(&work));
    }
}
