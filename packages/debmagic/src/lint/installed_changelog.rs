//! The installed Debian changelog of a Binary package, read from its Data files.
//!
//! Lintian picks the first of `changelog.Debian.gz`, `changelog.Debian`,
//! `changelog.debian.gz`, `changelog.debian`, `changelog.gz`, `changelog`
//! under `usr/share/doc/<pkg>` that is a file or a symlink, then parses the
//! resolved file (`gunzip` when the resolved name ends in `.gz`). The Binary
//! package's `changelog` in lintian, shared by the Rules through
//! `BinaryPackageContext::installed_changelog`.

use std::path::PathBuf;

use crate::lint::debian_changelog::DebianChangelog;
use crate::lint::file_tree::resolve_link;
use crate::lint::file_tree::{Entry, EntryKind, FileTree};

const CANDIDATES: &[&str] = &[
    "changelog.Debian.gz",
    "changelog.Debian",
    "changelog.debian.gz",
    "changelog.debian",
    "changelog.gz",
    "changelog",
];

pub(crate) struct InstalledChangelog {
    pub path: PathBuf,
    pub text: String,
    pub parsed: DebianChangelog,
}

/// The installed Debian changelog of the Binary package `package`, read from
/// its Data files `files`. `None` when there is none.
pub(crate) fn read_installed_changelog(
    files: &FileTree,
    package: &str,
) -> Option<InstalledChangelog> {
    let (path, entry) = candidate(files, package)?;
    let text = changelog_text(files, entry)?;
    let parsed = DebianChangelog::parse(&text);
    Some(InstalledChangelog { path, text, parsed })
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

pub(crate) fn changelog_text(files: &FileTree, entry: &Entry) -> Option<String> {
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
