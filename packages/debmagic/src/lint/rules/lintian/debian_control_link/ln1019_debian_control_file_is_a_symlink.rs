use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::SourceRule;

pub struct DebianControlFileIsASymlink;

declare_rule! {
    /// The `debian/control` file is a symbolic link.
    ///
    /// The `debian/` directory is resolved through symlinks first, as lintian
    /// does, so a symlinked `debian/` is followed. A `control` entry that is a
    /// symlink is reported, whatever its target. A missing `debian/` or
    /// `control`, or a `debian` that is not a directory, is skipped. Extra is
    /// empty. The Location is the `control` path with no line, under the
    /// resolved `debian/` directory.
    DebianControlFileIsASymlink,
    code = "LN1019",
    tag = "debian-control-file-is-a-symlink",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianControlFileIsASymlink {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(debian) = resolve(files, Path::new("debian")) else {
            return;
        };
        // A tree may omit directory entries, so only a non-directory stops here.
        if files.get(&debian).is_some_and(|entry| !entry.is_dir()) {
            return;
        }
        let control = debian.join("control");
        let is_symlink = files.get(&control).is_some_and(|entry| entry.is_symlink());
        if is_symlink {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: control,
                line: None,
                column: None,
            }));
        }
    }
}

/// Lintian's `resolve_path`: walks `path` and follows every symlink on the
/// way, including a final one. `None` when a link leaves the tree, is
/// absolute, loops, or dangles.
fn resolve(tree: &FileTree, path: &Path) -> Option<PathBuf> {
    const MAXIMUM_LINKS: usize = 8;
    let mut resolved = PathBuf::new();
    let mut pending: VecDeque<OsString> = path.iter().map(|part| part.to_os_string()).collect();
    let mut links = 0usize;
    while let Some(part) = pending.pop_front() {
        match part.as_os_str().to_str() {
            Some(".") => continue,
            Some("..") => {
                if !resolved.pop() {
                    return None;
                }
                continue;
            }
            _ => {}
        }
        resolved.push(&part);
        // Only links are followed. A missing component is left for the caller.
        let Some(entry) = tree.get(&resolved).filter(|entry| entry.is_symlink()) else {
            continue;
        };
        links += 1;
        if links > MAXIMUM_LINKS {
            return None;
        }
        let target = entry.link_target()?;
        if target.is_absolute() {
            return None;
        }
        resolved.pop();
        for piece in target.iter().rev() {
            pending.push_front(piece.to_os_string());
        }
    }
    Some(resolved)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            vec![("debian/control", TestFile::new("Source: example\n"))],
            // A symlink beside `control` is not `control`.
            vec![
                ("debian/control.real", TestFile::new("Source: example\n")),
                ("debian/control.gz", TestFile::symlink("control.real")),
            ],
            // A `control` directory is not a symlink.
            vec![("debian/control/x", TestFile::new("content"))],
            // No `debian/` directory at all.
            vec![("debian", TestFile::new("not a directory"))],
            // A `debian/` link that dangles has no directory to look in.
            vec![("debian", TestFile::symlink("nowhere"))],
        ];
        let fail = vec![
            vec![
                ("debian/control.real", TestFile::new("Source: example\n")),
                ("debian/control", TestFile::symlink("control.real")),
            ],
            // A dangling link is still a symlink.
            vec![("debian/control", TestFile::symlink("missing"))],
            // Through a symlinked `debian/`, the resolved `control` is reported.
            vec![
                ("real/control", TestFile::symlink("elsewhere")),
                ("real/control.real", TestFile::new("Source: example\n")),
                ("debian", TestFile::symlink("real")),
            ],
        ];
        SourceTreeTester::new(DebianControlFileIsASymlink, pass, fail).test_and_snapshot();
    }
}
