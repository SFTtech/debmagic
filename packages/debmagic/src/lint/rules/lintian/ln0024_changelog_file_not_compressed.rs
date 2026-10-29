use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct ChangelogFileNotCompressed;

declare_rule! {
    /// A changelog file in the package's doc directory is not gzip-compressed.
    ///
    /// Looks at the files and symlinks directly in `usr/share/doc/<pkg>`
    /// (skipped when that path is itself a symlink) whose name is
    /// `changelog`, `changelog.html`, or `changelog.Debian`, exactly in that
    /// case, and flags those without a `.gz` suffix. Udebs are checked too.
    /// Extra is empty; the Data files path is the Location.
    ChangelogFileNotCompressed,
    code = "LN0024",
    tag = "changelog-file-not-compressed",
    default_selected = true,
    experimental = false,
}

static CHANGELOG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^changelog(?:\.html|\.Debian)?(?:\.gz)?$").expect("valid regex"));

impl BinaryPackageRule for ChangelogFileNotCompressed {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let doc_dir = format!("usr/share/doc/{}", ctx.package_name());
        let Some(files) = ctx.data_files() else {
            return;
        };
        if files
            .get(&doc_dir)
            .is_some_and(|entry| entry.kind() == EntryKind::Symlink)
        {
            return;
        }
        let prefix = format!("{doc_dir}/");
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter_map(|entry| {
                let name = entry.path().to_str()?.strip_prefix(&prefix)?;
                (!name.contains('/') && CHANGELOG.is_match(name) && !name.ends_with(".gz"))
                    .then(|| entry.path().to_path_buf())
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

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("content")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            files(&[]),
            files(&[
                "usr/share/doc/example/changelog.gz",
                "usr/share/doc/example/changelog.Debian.gz",
                "usr/share/doc/example/changelog.html.gz",
            ]),
            // Other names and other directories are not changelog files.
            files(&["usr/share/doc/example/ChangeLog"]),
            files(&["usr/share/doc/example/changelog.debian"]),
            files(&["usr/share/doc/example/changelog.txt"]),
            files(&["usr/share/doc/example/sub/changelog"]),
            files(&["usr/share/doc/other/changelog"]),
            BinaryPackageCase::from(vec![
                ("usr/share/doc/example", TestFile::symlink("other")),
                ("usr/share/doc/other/changelog", TestFile::new("content")),
            ]),
        ];
        let fail = vec![
            files(&["usr/share/doc/example/changelog"]),
            files(&["usr/share/doc/example/changelog.Debian"]),
            files(&["usr/share/doc/example/changelog.html"]),
            files(&[
                "usr/share/doc/example/changelog",
                "usr/share/doc/example/changelog.Debian",
            ]),
            files(&["usr/share/doc/example/changelog"]).udeb(),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/changelog",
                TestFile::symlink("../other/changelog"),
            )]),
        ];
        BinaryPackageTester::new(ChangelogFileNotCompressed, pass, fail).test_and_snapshot();
    }
}
