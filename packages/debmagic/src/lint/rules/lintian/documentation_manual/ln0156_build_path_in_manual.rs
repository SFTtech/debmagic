use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::manual::{has_build_marker, nested_manual_path};

pub struct BuildPathInManual;

declare_rule! {
    /// A manual-page path contains a build directory.
    ///
    /// A file, hardlink, or symlink nested under `usr/share/man/` (not
    /// directly in that directory) whose path contains `/_build_` or
    /// `_tmp_buildd`. A section directory is not required. Udebs are skipped.
    /// Extra is empty; the Data files path is the Location.
    BuildPathInManual,
    code = "LN0156",
    tag = "build-path-in-manual",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BuildPathInManual {
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
            if has_build_marker(path) {
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

    const BUILD: &str =
        "usr/share/man/man1/_build_buildd-openchangex_1.0-3-i386-mbKpOA_openchange-1.0_simple.1.gz";
    const BUILD6: &str =
        "usr/share/man/man6/_build_buildd-openchangex_1.0-3-i386-mbKpOA_openchange-1.0_simple.6.gz";

    #[test]
    fn test() {
        let pass = vec![
            case(vec![]),
            case(vec![text("usr/share/man/man1/foo.1.gz")]),
            // The slash before `_build_` is required; `_tmp_buildd` needs its
            // leading underscore.
            case(vec![
                text("usr/share/man/man1/foo_build_bar.1.gz"),
                text("usr/share/man/man1/tmp_buildd.1.gz"),
            ]),
            case(vec![text("usr/share/man/_build_foo")]),
            case(vec![(
                "usr/share/man/man1/_build_dir".to_string(),
                TestFile::directory(),
            )]),
            case(vec![text(BUILD)]).udeb(),
        ];
        let fail = vec![
            case(vec![text(BUILD)]),
            case(vec![text(BUILD6)]),
            case(vec![text("usr/share/man/man1/foo_tmp_buildd.1.gz")]),
            case(vec![(
                "usr/share/man/man1/_build_linked.1.gz".to_string(),
                TestFile::symlink("foo.1.gz"),
            )]),
            case(vec![(
                "usr/share/man/man1/_tmp_buildd.1.gz".to_string(),
                TestFile::new("man").hardlink(),
            )]),
            // Not a section directory, still under `usr/share/man/`.
            case(vec![text("usr/share/man/foo/_build_bar")]),
        ];
        BinaryPackageTester::new(BuildPathInManual, pass, fail).test_and_snapshot();
    }
}
