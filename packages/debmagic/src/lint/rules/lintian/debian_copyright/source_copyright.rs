//! `debian/copyright` and `debian/<installable>.copyright` as the source Check
//! counts them: any child of the resolved `debian/` directory, including a
//! symlink. The directory itself is followed when it is a symlink.

use std::path::{Component, Path, PathBuf};

use crate::lint::context::SourceContext;
use crate::lint::file_tree::{EntryKind, FileTree};

/// Existing copyright candidates, or `None` when `debian/` does not resolve to
/// a directory.
pub(super) fn copyright_paths(ctx: &mut SourceContext<'_>) -> Option<Vec<PathBuf>> {
    let names: Vec<String> = ctx
        .debian_control()
        .map(|control| {
            control
                .installables()
                .iter()
                .map(|installable| format!("{}.copyright", installable.name()))
                .collect()
        })
        .unwrap_or_default();
    let files = ctx.files()?;
    let directory = resolve_directory(files, Path::new("debian"))?;
    let mut paths = Vec::new();
    for name in std::iter::once(String::from("copyright")).chain(names) {
        let path = directory.join(name);
        if files.get(&path).is_some() {
            paths.push(path);
        }
    }
    Some(paths)
}

fn resolve_directory(files: &FileTree, start: &Path) -> Option<PathBuf> {
    let mut path = start.to_path_buf();
    for _ in 0..32 {
        // A symlink to `.` resolves to the package root, which has no entry.
        if path.as_os_str().is_empty() {
            return Some(path);
        }
        let entry = files.get(&path)?;
        match entry.kind() {
            EntryKind::Directory => return Some(path),
            EntryKind::Symlink => {
                let target = entry.link_target()?;
                path = join_link(path.parent(), target);
            }
            _ => return None,
        }
    }
    None
}

fn join_link(parent: Option<&Path>, target: &Path) -> PathBuf {
    if let Ok(relative) = target.strip_prefix("/") {
        return relative.to_path_buf();
    }
    let mut path = parent.unwrap_or(Path::new("")).to_path_buf();
    for component in target.components() {
        match component {
            Component::ParentDir => {
                path.pop();
            }
            Component::Normal(name) => path.push(name),
            _ => {}
        }
    }
    path
}
