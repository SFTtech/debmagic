//! `README.Debian` lookup shared by the `debian/readme` Rules.
//!
//! The first readable name under `usr/share/doc/<package>/` wins, gzip names
//! are decompressed, and symlinks and other non-files are skipped, as in
//! lintian's `open_readme`.

use std::path::PathBuf;

use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::gzip::gunzip;

const README_NAMES: [&str; 4] = [
    "README.Debian.gz",
    "README.Debian",
    "README.debian.gz",
    "README.debian",
];

/// Path and decoded text of the `README.Debian` that lintian would open.
pub(super) fn readme_text(files: &FileTree, package: &str) -> Option<(PathBuf, String)> {
    for name in README_NAMES {
        let path = PathBuf::from(format!("usr/share/doc/{package}/{name}"));
        let Some(entry) = files.get(&path) else {
            continue;
        };
        if !is_file(entry) {
            continue;
        }
        let bytes = files.read(entry).ok()?;
        let text = if name.ends_with(".gz") {
            let plain = gunzip(&bytes)?;
            String::from_utf8_lossy(&plain).into_owned()
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        };
        return Some((path, text));
    }
    None
}

fn is_file(entry: &Entry) -> bool {
    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
}
