use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct SvnCommitFileInPackage;

declare_rule! {
    /// The package ships a leftover Subversion commit message file.
    ///
    /// Flags regular files whose path contains `svn-commit` followed by
    /// anything and ending in `.tmp`. Udebs are checked too. Extra is empty;
    /// the Data files path is the Location.
    SvnCommitFileInPackage,
    code = "LN0057",
    tag = "svn-commit-file-in-package",
    default_selected = true,
    experimental = false,
}

static SVN_COMMIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)svn-commit.*\.tmp$").expect("valid regex"));

impl BinaryPackageRule for SvnCommitFileInPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|path| SVN_COMMIT.is_match(path))
            })
            .map(|entry| entry.path().to_path_buf())
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

    fn tree(entries: &[(&str, TestFile)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let file = || TestFile::new("x");
        let pass = vec![
            tree(&[]),
            tree(&[("usr/share/x/svn-commit.txt", file())]),
            tree(&[("usr/share/x/svn-commit.tmp.bak", file())]),
            tree(&[("usr/share/x/commit.tmp", file())]),
            tree(&[("usr/share/x/svn-commit.tmp", TestFile::symlink("y"))]),
            tree(&[("usr/share/svn-commit.tmp", TestFile::directory())]),
            // Only svn-commit, not svk-commit.
            tree(&[("usr/share/x/svk-commitsEr9P.tmp", file())]),
        ];
        let fail = vec![
            tree(&[("usr/share/x/svn-commit.tmp", file())]),
            tree(&[("usr/share/x/svn-commit-yeehaa.tmp", file())]),
            tree(&[
                ("files/svn-commit.tmp", file()),
                ("svn-commit1.tmp", file()),
            ]),
            tree(&[("svn-commit.tmp", file())]).udeb(),
        ];
        BinaryPackageTester::new(SvnCommitFileInPackage, pass, fail).test_and_snapshot();
    }
}
