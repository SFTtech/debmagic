//! Paths and expected package names shared by the `desktop/gnome/gir` Rules.
//!
//! Lintian's Check walks the direct children of `usr/share/gir-1.0`,
//! `usr/lib/<triplet>/gir-1.0`, `usr/lib/girepository-1.0` and
//! `usr/lib/<triplet>/girepository-1.0`. Directories are left out here.

use std::path::Path;

use crate::lint::diagnostic::Location;
use crate::lint::file_tree::file_name_lossy;
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rules::lintian::fields_relations_more::architectures::deb_host_multiarch;

/// The `$DEB_HOST_MULTIARCH` text Lintian uses when the Architecture has no
/// triplet (`all`); the Check then looks for that literal directory name.
pub(super) const PLACEHOLDER_TRIPLET: &str = "${DEB_HOST_MULTIARCH}";

/// The `DEB_HOST_MULTIARCH` triplet of an Architecture, or the placeholder.
pub(super) fn triplet_or_placeholder(architecture: &str) -> &'static str {
    triplet(architecture).unwrap_or(PLACEHOLDER_TRIPLET)
}

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
        .filter(|entry| file_name_lossy(entry.path()).ends_with(".gir"))
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

/// Typelibs in the old `usr/lib/girepository-1.0` directory.
pub(super) fn old_typelib_files(files: &FileTree) -> Vec<&Entry> {
    children(files, "usr/lib/girepository-1.0")
}

/// Typelibs in the old directory, then in `usr/lib/<triplet>/girepository-1.0`.
pub(super) fn typelib_files<'a>(files: &'a FileTree, triplet: &str) -> Vec<&'a Entry> {
    let mut libs = old_typelib_files(files);
    libs.extend(children(
        files,
        &format!("usr/lib/{triplet}/girepository-1.0"),
    ));
    libs
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

/// The package name a typelib expects: `gir1.2-` and the lowercased file
/// name without `.typelib`, with `_` as `-`.
pub(super) fn typelib_package_stem(basename: &str) -> String {
    let name = format!("gir1.2-{}", basename.to_lowercase());
    let name = name.strip_suffix(".typelib").unwrap_or(&name);
    name.replace('_', "-")
}
