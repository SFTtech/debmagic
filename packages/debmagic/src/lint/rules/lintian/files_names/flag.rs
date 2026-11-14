//! Emit the empty-extra Diagnostic these Rules share.
//!
//! Location is the Data files path, with a trailing slash on directories.

use std::path::PathBuf;

use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::Severity;

use super::names::{NameHit, name_hits};

pub(super) fn flag_name(ctx: &mut BinaryPackageContext<'_>, hit: NameHit, severity: Severity) {
    let Some(files) = ctx.data_files() else {
        return;
    };
    let paths: Vec<PathBuf> = files
        .entries()
        .iter()
        .filter(|entry| name_hits(entry.path(), entry.kind()).contains(&hit))
        .map(|entry| {
            let mut path = entry.path().to_path_buf();
            if entry.kind() == EntryKind::Directory {
                path = PathBuf::from(format!("{}/", path.display()));
            }
            path
        })
        .collect();
    for path in paths {
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
}
