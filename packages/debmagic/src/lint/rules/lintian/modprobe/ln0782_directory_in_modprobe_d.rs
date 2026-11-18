use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::scope::in_scope;

pub struct DirectoryInModprobeD;

declare_rule! {
    /// A directory sits below `usr/lib/` or `etc/` `modprobe.d` or
    /// `modules-load.d`.
    ///
    /// Data files directories whose path is below one of those four
    /// directories are reported, because kmod does not read nested
    /// directories. Extra is empty; the path, with a trailing slash, is the
    /// Location. Udebs are checked too.
    DirectoryInModprobeD,
    code = "LN0782",
    tag = "directory-in-modprobe.d",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DirectoryInModprobeD {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::Directory))
            .filter(|entry| entry.path().to_str().is_some_and(in_scope))
            .map(|entry| PathBuf::from(format!("{}/", entry.path().display())))
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

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            tree(vec![("usr/lib/modprobe.d", TestFile::directory())]),
            tree(vec![("etc/modprobe.d/ok.conf", TestFile::new("x"))]),
            tree(vec![("etc/other.d/test.conf", TestFile::directory())]),
            tree(vec![("usr/lib/modprobe.dx/test", TestFile::directory())]),
            tree(vec![("usr/share/modprobe.d/test", TestFile::directory())]),
        ];
        let fail = vec![
            tree(vec![(
                "usr/lib/modprobe.d/test.conf",
                TestFile::directory(),
            )]),
            tree(vec![("etc/modules-load.d/sub", TestFile::directory())]),
            tree(vec![
                ("etc/modprobe.d", TestFile::directory()),
                ("etc/modprobe.d/nested", TestFile::directory()),
            ]),
            tree(vec![("etc/modprobe.d/nested", TestFile::directory())]).udeb(),
        ];
        BinaryPackageTester::new(DirectoryInModprobeD, pass, fail).test_and_snapshot();
    }
}
