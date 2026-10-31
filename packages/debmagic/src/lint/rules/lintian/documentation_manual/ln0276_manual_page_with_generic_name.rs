use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::manual::nested_manual_path;

pub struct ManualPageWithGenericName;

declare_rule! {
    /// A manual-page path contains `/README.`.
    ///
    /// A file, hardlink, or symlink nested under `usr/share/man/` (not
    /// directly in that directory). A section directory is not required. The
    /// match is case-sensitive, so `readme` is left alone. Udebs are skipped.
    /// Extra is empty; the Data files path is the Location.
    ManualPageWithGenericName,
    code = "LN0276",
    tag = "manual-page-with-generic-name",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ManualPageWithGenericName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            let Some(path) = nested_manual_path(entry) else {
                continue;
            };
            if path.contains("/README.") {
                hits.push(PathBuf::from(path));
            }
        }
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

    fn text(path: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::new("man"))
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn test() {
        let pass = vec![
            case(vec![]),
            case(vec![text("usr/share/man/man1/foo.1.gz")]),
            case(vec![text("usr/share/man/man1/readme.1.gz")]),
            case(vec![text("usr/share/man/man1/README")]),
            case(vec![text("usr/share/man/man1/XREADME.1.gz")]),
            // Directly in `usr/share/man/`, which returns before this Tag.
            case(vec![text("usr/share/man/README.1.gz")]),
            case(vec![text("usr/share/doc/README.txt")]),
            case(vec![(
                "usr/share/man/man1/README.dir".to_string(),
                TestFile::directory(),
            )]),
            case(vec![text("usr/share/man/man1/README.1.gz")]).udeb(),
        ];
        let fail = vec![case(vec![
            text("usr/share/man/man1/README.1.gz"),
            text("usr/share/man/foo/README.txt"),
            (
                "usr/share/man/man1/README.link".to_string(),
                TestFile::symlink("foo.1.gz"),
            ),
            (
                "usr/share/man/man1/README.hard".to_string(),
                TestFile::new("man").hardlink(),
            ),
        ])];
        BinaryPackageTester::new(ManualPageWithGenericName, pass, fail).test_and_snapshot();
    }
}
