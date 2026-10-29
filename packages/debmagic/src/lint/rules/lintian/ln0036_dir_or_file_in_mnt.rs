use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use std::path::PathBuf;

use crate::lint::file_tree::EntryKind;

pub struct DirOrFileInMnt;

declare_rule! {
    /// The package ships something under `/mnt`.
    ///
    /// Flags every file, directory and symlink whose path starts with
    /// `mnt/` followed by at least one more character; `/mnt` itself is
    /// not flagged. Udebs are checked too. Extra is empty; the Data files
    /// path (with a trailing slash for directories) is the Location.
    DirOrFileInMnt,
    code = "LN0036",
    tag = "dir-or-file-in-mnt",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DirOrFileInMnt {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
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
                    .is_some_and(|path| path.starts_with("mnt/") && path.len() > 4)
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
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
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
            tree(&[("usr/share/mnt", false), ("usr/mnt/x", false)]),
            tree(&[("mnt", true)]),
            tree(&[("mntx/file", false)]),
            tree(&[("var/mnt/file", false)]),
        ];
        let fail = vec![
            tree(&[("mnt/foo", true), ("mnt/foo/bar", false)]),
            tree(&[("mnt", true), ("mnt/file", false)]),
            tree(&[("mnt/file", false)]).udeb(),
        ];
        BinaryPackageTester::new(DirOrFileInMnt, pass, fail).test_and_snapshot();
    }
}
