use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsXvpicsDir;

declare_rule! {
    /// A directory named `.xvpics` below the top level.
    ///
    /// Only directories are matched (symlinks are not). The name must have a
    /// parent component, so a root `.xvpics` is not flagged, and the match is
    /// case-sensitive. Udebs are checked too. Extra is empty; the Location is
    /// the directory path with lintian's trailing `/`.
    PackageContainsXvpicsDir,
    code = "LN1095",
    tag = "package-contains-xvpics-dir",
    default_selected = true,
    experimental = false,
}

/// `m{/\.xvpics/?$}` on the raw bytes of the directory name.
static XVPICS_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)/\.xvpics/?$").expect("valid regex"));

impl BinaryPackageRule for PackageContainsXvpicsDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .query()
            .kinds([EntryKind::Directory])
            .into_iter()
            .filter(|entry| XVPICS_DIR.is_match(entry.path().as_os_str().as_encoded_bytes()))
            .map(|entry| {
                // Lintian's name for a directory ends in `/`.
                let mut path = entry.path().as_os_str().to_owned();
                if !path.as_encoded_bytes().ends_with(b"/") {
                    path.push("/");
                }
                PathBuf::from(path)
            })
            .collect();
        for path in hits {
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

    fn file(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), file)])
    }

    #[test]
    fn test() {
        let text = TestFile::new("x");
        let pass = vec![
            file(".xvpics", TestFile::directory()),
            file("usr/share/doc/.xvpics.d", TestFile::directory()),
            file("usr/share/doc/xvpics", TestFile::directory()),
            file("usr/share/doc/.XVpics", TestFile::directory()),
            file("usr/share/doc/.xvpics", text.clone()),
            file("usr/share/doc/.xvpics", TestFile::symlink("elsewhere")),
        ];
        let fail = vec![
            file(
                "usr/share/doc/files-foo-in-bar/.xvpics",
                TestFile::directory(),
            ),
            file(
                "usr/share/doc/files-foo-in-bar/.xvpics/",
                TestFile::directory(),
            ),
            file("usr/share/pkg/.xvpics", TestFile::directory()).udeb(),
        ];
        BinaryPackageTester::new(PackageContainsXvpicsDir, pass, fail).test_and_snapshot();
    }
}
