use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use std::path::PathBuf;

use crate::lint::file_tree::EntryKind;

pub struct DirOrFileInSrv;

declare_rule! {
    /// The package ships something under `/srv`.
    ///
    /// Flags every file, directory and symlink whose path starts with
    /// `srv/` followed by at least one more character; `/srv` itself is
    /// not flagged. Udebs are checked too. Extra is empty; the Data files
    /// path (with a trailing slash for directories) is the Location.
    DirOrFileInSrv,
    code = "LN0035",
    tag = "dir-or-file-in-srv",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DirOrFileInSrv {
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
                    .is_some_and(|path| path.starts_with("srv/") && path.len() > 4)
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
            tree(&[("usr/share/srv", false), ("usr/srv/x", false)]),
            tree(&[("srv", true)]),
            tree(&[("srvx/file", false)]),
            tree(&[("var/srv/file", false)]),
        ];
        let fail = vec![
            tree(&[("srv/foo", true), ("srv/foo/bar", false)]),
            tree(&[("srv", true), ("srv/file", false)]),
            tree(&[("srv/file", false)]).udeb(),
        ];
        BinaryPackageTester::new(DirOrFileInSrv, pass, fail).test_and_snapshot();
    }
}
