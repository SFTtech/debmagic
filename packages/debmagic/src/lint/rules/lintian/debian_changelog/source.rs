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
/// else a 1.0-style package. A Source package with a Dsc is native when
/// `Files` has no `{name}_{version}.diff.gz`. A Source tree has no Dsc, so a
/// hyphen after the epoch stands in for the diff `dpkg-source` would build.
pub(super) fn package_is_native(ctx: &mut SourceContext<'_>) -> bool {
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
        None => !strip_epoch(&version).contains('-'),
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
