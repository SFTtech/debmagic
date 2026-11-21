//! Paths and expected package names shared by the `desktop/gnome/gir` Rules.
//!
//! Lintian's Check walks the direct children of `usr/share/gir-1.0`,
//! `usr/lib/<triplet>/gir-1.0`, `usr/lib/girepository-1.0` and
//! `usr/lib/<triplet>/girepository-1.0`. Directories are left out here.

use std::path::Path;

use crate::lint::diagnostic::Location;
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rules::lintian::fields_relations_more::architectures::deb_host_multiarch;

/// The triplet of an Architecture, `None` when it has none.
pub(super) fn triplet(architecture: &str) -> Option<&'static str> {
    deb_host_multiarch(architecture)
}

/// Direct children of `dir` that are not directories.
fn children<'a>(files: &'a FileTree, dir: &str) -> Vec<&'a Entry> {
    let dir = Path::new(dir);
    files
        .entries()
        .iter()
        .filter(|entry| entry.path().parent() == Some(dir))
        .filter(|entry| entry.kind() != EntryKind::Directory)
        .collect()
}

/// Direct children of `dir` whose names end in `.gir`.
fn girs_in<'a>(files: &'a FileTree, dir: &str) -> Vec<&'a Entry> {
    children(files, dir)
        .into_iter()
        .filter(|entry| basename(entry).ends_with(".gir"))
        .collect()
}

/// GIR XML files: `usr/share/gir-1.0/*.gir`, then `usr/lib/<triplet>/gir-1.0/*.gir`
/// when the Architecture has a triplet.
pub(super) fn gir_files<'a>(files: &'a FileTree, triplet: Option<&str>) -> Vec<&'a Entry> {
    let mut girs = girs_in(files, "usr/share/gir-1.0");
    if let Some(triplet) = triplet {
        girs.extend(girs_in(files, &format!("usr/lib/{triplet}/gir-1.0")));
    }
    girs
}

/// The file name of an Entry, lossily.
pub(super) fn basename(entry: &Entry) -> String {
    entry
        .path()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The Location of an Entry: its path, no line.
pub(super) fn location(entry: &Entry) -> Location {
    Location {
        path: entry.path().to_path_buf(),
        line: None,
        column: None,
    }
}

/// The package name a GIR file expects, without `-dev`: `gir1.2-` and the
/// lowercased file name without `.gir`, with `_` as `-`.
pub(super) fn gir_package_stem(basename: &str) -> String {
    let name = format!("gir1.2-{}", basename.to_lowercase());
    let name = name.strip_suffix(".gir").unwrap_or(&name);
    name.replace('_', "-")
}
