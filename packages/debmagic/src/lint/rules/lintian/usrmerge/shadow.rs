//! Data files that also exist under `usr/`, shared by the `usrmerge` Check's
//! Rules.

use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;

/// A root-level entry (`bin/`, `sbin/`, `lib*/`) and its `usr/` twin.
pub(super) struct Shadowed {
    /// The root entry; directories end in a slash.
    pub(super) location: PathBuf,
    /// The `usr/` entry's path, without a trailing slash.
    pub(super) usr: PathBuf,
    /// The root path is a shared library or static archive.
    pub(super) library: bool,
}

static ROOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:s?bin|lib(?:|[ox]?32|64))/").expect("valid regex"));

static LIBRARY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^lib.+\.(?:so[\.0-9]*|a)$").expect("valid regex"));

/// Entries under `bin/`, `sbin/`, `lib/`, `lib32/`, `libo32/`, `libx32/` or
/// `lib64/` whose `usr/` twin exists. A directory whose twin is also a
/// directory is not reported.
pub(super) fn shadowed(ctx: &mut BinaryPackageContext<'_>) -> Vec<Shadowed> {
    let Some(files) = ctx.data_files() else {
        return Vec::new();
    };
    files
        .entries()
        .iter()
        .filter_map(|entry| {
            let path = entry.path().to_str()?;
            if !ROOT.is_match(path) {
                return None;
            }
            let twin = files.get(format!("usr/{path}"))?;
            let both_dirs = matches!(entry.kind(), EntryKind::Directory)
                && matches!(twin.kind(), EntryKind::Directory);
            if both_dirs {
                return None;
            }
            let location = match entry.kind() {
                EntryKind::Directory => PathBuf::from(format!("{path}/")),
                _ => PathBuf::from(path),
            };
            Some(Shadowed {
                location,
                usr: twin.path().to_path_buf(),
                library: LIBRARY.is_match(path),
            })
        })
        .collect()
}
