//! Emit the empty-extra Diagnostic these Rules share.
//!
//! Location is the Data files path, with a trailing slash on directories.
//! Line and column stay absent.

use std::path::PathBuf;

use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::Severity;

/// `prefix` includes the trailing slash (`"srv/"`, `"mnt/"`). The directory
/// itself is not flagged. Udebs are checked. The hierarchy chain is not
/// consulted.
pub(super) fn flag_prefix(ctx: &mut BinaryPackageContext<'_>, prefix: &str) {
    let Some(files) = ctx.data_files() else {
        return;
    };
    let paths: Vec<PathBuf> = files
        .entries()
        .iter()
        .filter(|entry| {
            entry
                .path()
                .to_str()
                .is_some_and(|path| path.starts_with(prefix) && path.len() > prefix.len())
        })
        .map(|entry| {
            let mut path = entry.path().to_path_buf();
            if entry.kind() == EntryKind::Directory {
                path = PathBuf::from(format!("{}/", path.display()));
            }
            path
        })
        .collect();
    for path in paths {
        emit(ctx, path);
    }
}

pub(super) fn emit(ctx: &mut BinaryPackageContext<'_>, path: PathBuf) {
    emit_with(ctx, path, Severity::Error);
}

pub(super) fn emit_with(ctx: &mut BinaryPackageContext<'_>, path: PathBuf, severity: Severity) {
    ctx.diagnostic(
        Diagnostic::error("")
            .with_severity(severity)
            .with_location(Location {
                path,
                line: None,
                column: None,
            }),
    );
}
