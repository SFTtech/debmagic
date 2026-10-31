//! The installed Debian changelog of a Binary package, read from Data files.
//!
//! Lintian picks the first of `changelog.Debian.gz`, `changelog.Debian`,
//! `changelog.debian.gz`, `changelog.debian`, `changelog.gz`, `changelog`
//! under `usr/share/doc/<pkg>` that is a file or a symlink, then parses the
//! resolved file (`gunzip` when the resolved name ends in `.gz`).

use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use chrono::{DateTime, FixedOffset};
use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_changelog::DebianChangelog;
use crate::lint::file_tree::{Entry, EntryKind, FileTree};

const CANDIDATES: &[&str] = &[
    "changelog.Debian.gz",
    "changelog.Debian",
    "changelog.debian.gz",
    "changelog.debian",
    "changelog.gz",
    "changelog",
];

const MAXIMUM_WIDTH: usize = 82;

const INVALID_DATES: &[&str] = &[
    r"Monday",
    r"Tues(day)?",
    r"Wednesday",
    r"Thurs(day)?",
    r"Friday",
    r"Saturday",
    r"Sunday",
    r"January",
    r"February",
    r"March",
    r"April",
    r"June",
    r"July",
    r"August",
    r"Sept(ember)?",
    r"October",
    r"November",
    r"December",
];

static LONG_LINE_OK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\s.o*+-]*(?:[Ss]ee:?\s+)?\S+$").expect("valid regex"));
static CLOSES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)closes:\s*(?:bug)?\#?\s?\d+(?:,\s*(?:bug)?\#?\s?\d+)*").expect("valid regex")
});

pub(super) struct InstalledChangelog {
    pub path: PathBuf,
    pub text: String,
    pub parsed: DebianChangelog,
}

pub(super) fn installed_changelog(
    ctx: &mut BinaryPackageContext<'_>,
) -> Option<InstalledChangelog> {
    let package = ctx.package_name().to_string();
    let files = ctx.data_files()?;
    let (path, entry) = candidate(files, &package)?;
    let text = changelog_text(files, entry)?;
    let parsed = DebianChangelog::parse(&text);
    Some(InstalledChangelog { path, text, parsed })
}

/// Installed `NEWS.Debian.gz` (that exact name), gunzipped when the resolved
/// file name ends in `.gz`.
pub(super) fn installed_news(ctx: &mut BinaryPackageContext<'_>) -> Option<InstalledChangelog> {
    let package = ctx.package_name().to_string();
    let files = ctx.data_files()?;
    let path = PathBuf::from(format!("usr/share/doc/{package}/NEWS.Debian.gz"));
    let entry = files.get(&path)?;
    if !matches!(
        entry.kind(),
        EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
    ) {
        return None;
    }
    let text = changelog_text(files, entry)?;
    let parsed = DebianChangelog::parse(&text);
    Some(InstalledChangelog { path, text, parsed })
}

/// Children of `usr/share/doc/<pkg>` whose basename is `NEWS.Debian` with an
/// optional `.gz`, case-insensitively, compressed, and not exactly
/// `NEWS.Debian.gz`.
pub(super) fn wrong_debian_news_paths(files: &FileTree, package: &str) -> Vec<PathBuf> {
    let dir = PathBuf::from(format!("usr/share/doc/{package}"));
    files
        .entries()
        .iter()
        .filter_map(|entry| {
            if entry.path().parent() != Some(dir.as_path()) {
                return None;
            }
            if !matches!(
                entry.kind(),
                EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
            ) {
                return None;
            }
            let name = entry.path().file_name()?.to_str()?;
            let folded = name.to_ascii_lowercase();
            let news = folded == "news.debian" || folded == "news.debian.gz";
            if news && name.ends_with(".gz") && name != "NEWS.Debian.gz" {
                Some(entry.path().to_path_buf())
            } else {
                None
            }
        })
        .collect()
}

fn candidate<'a>(files: &'a FileTree, package: &str) -> Option<(PathBuf, &'a Entry)> {
    let dir = format!("usr/share/doc/{package}");
    for name in CANDIDATES {
        let path = PathBuf::from(format!("{dir}/{name}"));
        let Some(entry) = files.get(&path) else {
            continue;
        };
        if matches!(
            entry.kind(),
            EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
        ) {
            return Some((path, entry));
        }
    }
    None
}

fn changelog_text(files: &FileTree, entry: &Entry) -> Option<String> {
    let resolved = resolve(files, entry)?;
    if !matches!(
        resolved.kind(),
        EntryKind::RegularFile | EntryKind::Hardlink
    ) {
        return None;
    }
    let bytes = files.read(resolved).ok()?;
    let plain = if resolved
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".gz"))
    {
        crate::lint::gzip::gunzip(&bytes)?
    } else {
        bytes
    };
    String::from_utf8(plain).ok()
}

fn resolve<'a>(files: &'a FileTree, entry: &'a Entry) -> Option<&'a Entry> {
    let mut current = entry;
    for _ in 0..8 {
        if current.kind() != EntryKind::Symlink {
            return Some(current);
        }
        let target = current.link_target()?;
        let next = resolve_link(current.path(), target)?;
        current = files.get(&next)?;
    }
    None
}

fn resolve_link(from: &Path, target: &Path) -> Option<PathBuf> {
    if target.is_absolute() {
        return None;
    }
    let mut resolved = from.parent().unwrap_or(Path::new("")).to_path_buf();
    for component in target.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(resolved)
}

/// `check_dch`: count lines until the first `^\s+\S`, inclusive.
pub(super) fn real_start(text: &str) -> u32 {
    let mut real_start = 0u32;
    let mut saw_tab_lead = false;
    for line in content_lines(text) {
        if !saw_tab_lead {
            real_start += 1;
        }
        if tab_lead(line) {
            saw_tab_lead = true;
        }
    }
    real_start
}

fn tab_lead(line: &str) -> bool {
    let rest = line.trim_start();
    !rest.is_empty() && rest.len() < line.len()
}

fn content_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

/// Line numbers of the newest entry's change lines that are at least 82
/// characters (the parser's extra leading space included) and are not a
/// bullet plus a single URL.
pub(super) fn long_lines(text: &str, changes: &str) -> Vec<u32> {
    let mut hits = Vec::new();
    for (position, line) in (real_start(text)..).zip(content_lines(changes)) {
        if line.chars().count() >= MAXIMUM_WIDTH && !LONG_LINE_OK.is_match(line) {
            hits.push(position);
        }
    }
    hits
}

pub(super) fn closes_any(changes: &str) -> bool {
    CLOSES.is_match(changes)
}

/// `debian-changelog-has-wrong-day-of-week` extra, when the declared weekday
/// is not the day of `timestamp` in the trailer offset. Invalid spelled-out
/// dates are left to `invalid-date-in-debian-changelog`.
pub(super) fn wrong_weekday(date: &str, timestamp: i64) -> Option<String> {
    if timestamp == 0 || invalid_changelog_date(date) {
        return None;
    }
    let (declared, rest) = date.split_once(',')?;
    let rest = rest.trim_start();
    if rest.is_empty() {
        return None;
    }
    let tz = rest.rsplit(' ').next()?;
    if !rest.contains(' ') {
        return None;
    }
    let offset = offset_seconds(tz)?;
    let zone = FixedOffset::east_opt(offset)?;
    let when = DateTime::from_timestamp(timestamp, 0)?.with_timezone(&zone);
    let actual = when.format("%a").to_string();
    if declared != actual {
        Some(format!(
            "{} was a {}",
            when.format("%Y-%m-%d"),
            when.format("%A")
        ))
    } else {
        None
    }
}

fn invalid_changelog_date(date: &str) -> bool {
    static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        INVALID_DATES
            .iter()
            .map(|pattern| {
                Regex::new(&format!(r"(?i)\b(?:{pattern})\b")).expect("invalid date pattern")
            })
            .collect()
    });
    PATTERNS.iter().any(|pattern| pattern.is_match(date))
}

fn offset_seconds(tz: &str) -> Option<i32> {
    if matches!(tz, "UTC" | "GMT" | "UT" | "Z") {
        return Some(0);
    }
    let (sign, rest) = if let Some(rest) = tz.strip_prefix('+') {
        (1, rest)
    } else {
        let rest = tz.strip_prefix('-')?;
        (-1, rest)
    };
    if rest.len() != 4 || !rest.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let hours: i32 = rest[..2].parse().ok()?;
    let mins: i32 = rest[2..].parse().ok()?;
    Some(sign * (hours * 3600 + mins * 60))
}
