use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::ln0024_changelog_file_not_compressed::doc_changelog_files;

pub struct DebianChangelogFileIsASymlink;

declare_rule! {
    /// A changelog file in the package's doc directory is a symlink.
    ///
    /// Looks at the files and symlinks directly in `usr/share/doc/<pkg>`
    /// (skipped when that path is itself a symlink) whose name is
    /// `changelog`, `changelog.html`, or `changelog.Debian`, with or without
    /// `.gz`, exactly in that case, and flags the symlinks. Udebs are checked
    /// too. Extra is empty; the Data files path is the Location.
    DebianChangelogFileIsASymlink,
    code = "LN0033",
    tag = "debian-changelog-file-is-a-symlink",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebianChangelogFileIsASymlink {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for (path, kind) in doc_changelog_files(ctx) {
            if kind == EntryKind::Symlink {
                ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                    path,
                    line: None,
                    column: None,
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(files: &[(&str, TestFile)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let regular = || TestFile::new("content");
        let pass = vec![
            files(&[]),
            files(&[("usr/share/doc/example/changelog.gz", regular())]),
            files(&[("usr/share/doc/example/changelog.Debian.gz", regular())]),
            // Other names and other directories are not changelog files.
            files(&[("usr/share/doc/example/ChangeLog", TestFile::symlink("x"))]),
            files(&[(
                "usr/share/doc/example/changelog.txt",
                TestFile::symlink("x"),
            )]),
            files(&[(
                "usr/share/doc/example/sub/changelog.gz",
                TestFile::symlink("x"),
            )]),
            files(&[("usr/share/doc/other/changelog.gz", TestFile::symlink("x"))]),
            files(&[
                ("usr/share/doc/example", TestFile::symlink("other")),
                ("usr/share/doc/example/changelog.gz", TestFile::symlink("x")),
            ]),
        ];
        let fail = vec![
            files(&[(
                "usr/share/doc/example/changelog.gz",
                TestFile::symlink("../other/changelog.gz"),
            )]),
            files(&[("usr/share/doc/example/changelog", TestFile::symlink("x"))]),
            files(&[(
                "usr/share/doc/example/changelog.Debian.gz",
                TestFile::symlink("x"),
            )]),
            files(&[(
                "usr/share/doc/example/changelog.html",
                TestFile::symlink("x"),
            )]),
            files(&[
                ("usr/share/doc/example/changelog.gz", TestFile::symlink("x")),
                (
                    "usr/share/doc/example/changelog.Debian.gz",
                    TestFile::symlink("y"),
                ),
            ]),
            files(&[("usr/share/doc/example/changelog.gz", TestFile::symlink("x"))]).udeb(),
        ];
        BinaryPackageTester::new(DebianChangelogFileIsASymlink, pass, fail).test_and_snapshot();
    }
}
