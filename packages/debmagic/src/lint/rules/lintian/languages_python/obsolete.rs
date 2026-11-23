//! Facts that the Rules porting lintian's `languages/python/obsolete` Check
//! share: whether a `debian/` file of the patched tree is a file once symlinks
//! inside the tree are followed, as lintian's `resolve_path` and `is_file`.

use std::path::{Component, Path, PathBuf};

use crate::lint::file_tree::{Entry, FileTree};

/// Symlinks followed before giving up, as the other resolvers do.
const MAX_SYMLINK_HOPS: usize = 8;

/// `path` resolves to a regular file or hardlink. A symlink whose target is
/// absolute, leaves the tree, or does not resolve is not a file.
pub(super) fn resolves_to_file(files: &FileTree, path: &str) -> bool {
    let Some(mut current) = files.get(path) else {
        return false;
    };
    for _ in 0..MAX_SYMLINK_HOPS {
        if !current.is_symlink() {
            return current.is_file();
        }
        let Some(next) = follow(files, current) else {
            return false;
        };
        current = next;
    }
    false
}

fn follow<'a>(files: &'a FileTree, link: &Entry) -> Option<&'a Entry> {
    let target = link.link_target()?;
    if target.is_absolute() {
        return None;
    }
    let joined = link.path().parent()?.join(target);
    files.get(normalize(&joined)?)
}

fn normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(out)
}
