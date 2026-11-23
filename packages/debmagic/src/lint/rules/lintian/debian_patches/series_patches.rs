//! The patches the plain Quilt series lists, shared by the Tags of the
//! `debian/patches/quilt` Check that read them.
//!
//! Mirrors the gates of lintian's `source` sub before it loops over the
//! series: Format `3.x (quilt)` or Build-Depends-All satisfying `quilt`, and
//! an open `debian/patches/series`. Names are read by [`QuiltSeries`]
//! (comments stripped, options dropped), in series order. A name is resolved
//! under `debian/patches/` the way lintian's `resolve_path` does, following
//! symlinks inside the tree.

use std::path::{Component, Path, PathBuf};

use crate::lint::context::{SourceContext, follow_symlinks};
use crate::lint::file_tree::{Entry, FileTree};

use super::ln0396_format_3_0_but_debian_changes_patch::{is_quilt_format, source_format};
use super::ln0706_patch_file_present_but_not_mentioned_in_series::BUILD_DEPENDS_ALL;

const SERIES: &str = "debian/patches/series";
const PATCH_DIR: &str = "debian/patches";

/// The plain series, when the Quilt part of the Check runs and the series
/// is open: its path (`debian/patches/series`) and the patch names it lists,
/// in series order and as written. `None` otherwise.
pub(super) fn plain_series(ctx: &mut SourceContext<'_>) -> Option<(PathBuf, Vec<String>)> {
    let quilt_format = is_quilt_format(&source_format(ctx));
    let builds_with_quilt = ctx.debian_control().is_some_and(|control| {
        control
            .source()
            .relations_and(BUILD_DEPENDS_ALL)
            .satisfies("quilt")
    });
    if !quilt_format && !builds_with_quilt {
        return None;
    }
    let series_path = Path::new(SERIES);
    let (path, series) = ctx
        .quilt_series()
        .into_iter()
        .find(|(path, _)| path == series_path)?;
    let names = series
        .patches()
        .iter()
        .map(|patch| patch.name().to_string())
        .collect();
    Some((path, names))
}

/// The file `name` names under `debian/patches/`, once its symlinks are
/// followed, when that is an open regular file or hardlink. `None` when it
/// is missing, dangling, escapes the tree, is not a file, or is not open.
pub(super) fn listed_patch<'t>(files: &'t FileTree, name: &str) -> Option<&'t Entry> {
    let joined = Path::new(PATCH_DIR).join(name);
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    follow_symlinks(files, files.get(&normalized)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(entries: Vec<Entry>) -> FileTree {
        FileTree::from_entries(None, entries)
    }

    #[test]
    fn names_resolve_under_patch_dir() {
        let files = tree(vec![
            Entry::regular_file("debian/patches/a.patch", None, None),
            Entry::regular_file("debian/other.patch", None, None),
            Entry::symlink("debian/patches/link.patch", "a.patch"),
            Entry::symlink("debian/patches/dangling.patch", "missing"),
            Entry::symlink("debian/patches/escape.patch", "../../outside"),
        ]);
        assert_eq!(
            listed_patch(&files, "a.patch").map(Entry::path),
            Some(Path::new("debian/patches/a.patch"))
        );
        assert_eq!(
            listed_patch(&files, "./a.patch").map(Entry::path),
            Some(Path::new("debian/patches/a.patch"))
        );
        assert_eq!(
            listed_patch(&files, "link.patch").map(Entry::path),
            Some(Path::new("debian/patches/a.patch"))
        );
        assert!(listed_patch(&files, "dangling.patch").is_none());
        assert!(listed_patch(&files, "escape.patch").is_none());
        assert_eq!(
            listed_patch(&files, "../other.patch").map(Entry::path),
            Some(Path::new("debian/other.patch"))
        );
        assert!(listed_patch(&files, "missing.patch").is_none());
    }
}
