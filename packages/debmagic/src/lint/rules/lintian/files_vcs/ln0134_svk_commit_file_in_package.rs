use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct SvkCommitFileInPackage;

declare_rule! {
    /// The package ships a leftover SVK commit message file.
    ///
    /// A regular file or hardlink whose path contains `svk-commit`, then at
    /// least one character, then `.tmp`. Extra is empty; the path is the
    /// Location. Udebs are checked too.
    SvkCommitFileInPackage,
    code = "LN0134",
    tag = "svk-commit-file-in-package",
    default_selected = true,
    experimental = false,
}

static SVK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"svk-commit.+\.tmp$").expect("valid regex"));

impl BinaryPackageRule for SvkCommitFileInPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter_map(|entry| {
                let path = entry.path().to_str()?;
                SVK.is_match(path).then_some(PathBuf::from(path))
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

    fn file(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("x"))])
    }

    #[test]
    fn test() {
        let pass = vec![file("svk-commit.tmp"), file("svn-commit.tmp")];
        let fail = vec![
            file("usr/share/foo/svk-commit-yeehaa.tmp"),
            file("files/svk-commitsEr9P.tmp"),
        ];
        BinaryPackageTester::new(SvkCommitFileInPackage, pass, fail).test_and_snapshot();
    }
}
