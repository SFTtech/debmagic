//! Location shape these Rules share.
//!
//! Directory index names end in `/`, the form lintian stores and sorts.
//! Extra is empty. Non-UTF-8 paths are left out (`Path::to_str`).

use std::path::PathBuf;

use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::Severity;

pub(super) fn emit(
    ctx: &mut BinaryPackageContext<'_>,
    path: impl Into<PathBuf>,
    severity: Severity,
) {
    ctx.diagnostic(
        Diagnostic::error("")
            .with_severity(severity)
            .with_location(Location {
                path: path.into(),
                line: None,
                column: None,
            }),
    );
}

/// Regular files and hardlinks. Lintian's `is_file` is those two; symlinks
/// and directories are not.
pub(super) fn is_file(entry: &Entry) -> bool {
    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
}

/// Installed-file names in lintian's visit order (`name cmp`, directories
/// keep their trailing slash).
pub(super) fn visit(files: &FileTree) -> Vec<(&Entry, String)> {
    let mut rows: Vec<_> = files
        .entries()
        .iter()
        .filter_map(|entry| lintian_name(entry).map(|name| (entry, name)))
        .collect();
    rows.sort_by(|left, right| left.1.cmp(&right.1));
    rows
}

fn lintian_name(entry: &Entry) -> Option<String> {
    let raw = entry.path().to_str()?;
    if entry.kind() == EntryKind::Directory && !raw.ends_with('/') {
        Some(format!("{raw}/"))
    } else {
        Some(raw.to_string())
    }
}
