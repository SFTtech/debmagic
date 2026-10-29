use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::EntryKind;

pub struct PackageContainsVcsControlDir;

declare_rule! {
    /// The package ships a directory that version control tools create.
    ///
    /// Flags directories (not files or symlinks) whose name, below at least
    /// one parent, is `CVS`, `.svn`, `.bzr`, `.git`, `.hg`, `.arch-ids`, or
    /// `{arch}`. Udebs are checked too. Extra is empty; the Data files path,
    /// with a trailing slash, is the Location.
    PackageContainsVcsControlDir,
    code = "LN0034",
    tag = "package-contains-vcs-control-dir",
    default_selected = true,
    experimental = false,
}

static VCS_DIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"/(?:CVS|\.(?:svn|bzr|git|hg)|\.arch-ids|\{arch\})$").expect("valid regex")
});

impl BinaryPackageRule for PackageContainsVcsControlDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| {
                entry.kind() == EntryKind::Directory
                    && entry
                        .path()
                        .to_str()
                        .is_some_and(|path| VCS_DIR.is_match(path))
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
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(entries: &[(&str, bool)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, dir)| {
                    let file = if *dir {
                        TestFile::directory()
                    } else {
                        TestFile::new("x")
                    };
                    (path.to_string(), file)
                })
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&[("usr/share/example/data", true)]),
            // At the root there is no parent segment.
            tree(&[(".git", true), ("CVS", true)]),
            // Files are not directories.
            tree(&[("usr/share/example/.git", false)]),
            tree(&[("usr/share/example/CVS", false)]),
            // Only the exact names.
            tree(&[
                ("usr/share/example/.gitignore", true),
                ("usr/share/example/CVSROOT", true),
            ]),
            tree(&[
                ("usr/share/example/cvs", true),
                ("usr/share/example/.GIT", true),
            ]),
            tree(&[("usr/share/example/.git/config", false)]),
        ];
        let fail = vec![
            tree(&[("usr/share/example/.git", true)]),
            tree(&[("usr/share/example/.svn", true)]),
            tree(&[
                ("usr/share/example/.bzr", true),
                ("usr/share/example/.hg", true),
            ]),
            tree(&[("usr/share/example/CVS", true)]),
            tree(&[
                ("usr/share/example/.arch-ids", true),
                ("usr/share/example/{arch}", true),
            ]),
            tree(&[("usr/share/example/.git", true)]).udeb(),
        ];
        BinaryPackageTester::new(PackageContainsVcsControlDir, pass, fail).test_and_snapshot();
    }
}
