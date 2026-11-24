use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::ln0396_format_3_0_but_debian_changes_patch::{is_quilt_format, source_format};
use super::ln0706_patch_file_present_but_not_mentioned_in_series::BUILD_DEPENDS_ALL;

pub struct QuiltSeriesButNoBuildDep;

declare_rule! {
    /// A `debian/patches/series` file in a source package whose
    /// `Build-Depends-All` of Debian control does not satisfy `quilt`.
    ///
    /// Skipped when Format is `3.x (quilt)` (Dsc, else
    /// `debian/source/format`). The series counts when it is a regular file,
    /// hardlink, or a symlink that resolves to one inside the tree; a
    /// directory or dangling symlink is skipped. Vendor series such as
    /// `<vendor>.series` are not checked. Extra is empty. The Location is the
    /// series file. Applies to the Source tree and Source package.
    QuiltSeriesButNoBuildDep,
    code = "LN1208",
    tag = "quilt-series-but-no-build-dep",
    default_selected = true,
    experimental = false,
}

impl SourceRule for QuiltSeriesButNoBuildDep {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_quilt_format(&source_format(ctx)) {
            return;
        }
        let builds_with_quilt = ctx.debian_control().is_some_and(|control| {
            control
                .source()
                .relations_and(BUILD_DEPENDS_ALL)
                .satisfies("quilt")
        });
        if builds_with_quilt {
            return;
        }
        let series_path = Path::new("debian/patches/series");
        if let Some((path, _)) = ctx
            .quilt_series()
            .into_iter()
            .find(|(path, _)| path == series_path)
        {
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
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>
Build-Depends: debhelper-compat (= 13)

Package: example
Architecture: all
Description: example
 extra
";

    const CONTROL_QUILT: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>
Build-Depends: debhelper-compat (= 13), quilt

Package: example
Architecture: all
Description: example
 extra
";

    fn tree(format: &str, control: &str, extra: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        let mut files = vec![
            (
                "debian/source/format".to_string(),
                TestFile::new(format!("{format}\n")),
            ),
            ("debian/control".to_string(), TestFile::new(control)),
        ];
        for (path, file) in extra {
            files.push(((*path).to_string(), file.clone()));
        }
        files
    }

    #[test]
    fn source() {
        let pass = vec![
            // Quilt format needs no build dependency.
            tree(
                "3.0 (quilt)",
                CONTROL,
                &[("debian/patches/series", TestFile::new("a.patch\n"))],
            ),
            // quilt is a build dependency.
            tree(
                "1.0",
                CONTROL_QUILT,
                &[("debian/patches/series", TestFile::new("a.patch\n"))],
            ),
            // No series file at all.
            tree("1.0", CONTROL, &[]),
            // A vendor series is not the plain series.
            tree(
                "1.0",
                CONTROL,
                &[("debian/patches/vendor.series", TestFile::new("a.patch\n"))],
            ),
            // A directory is not a series file.
            tree(
                "1.0",
                CONTROL,
                &[("debian/patches/series", TestFile::directory())],
            ),
            // A dangling symlink is not open.
            tree(
                "1.0",
                CONTROL,
                &[("debian/patches/series", TestFile::symlink("missing"))],
            ),
        ];
        let fail = vec![
            // Format 1.0 with a series and no quilt build dependency.
            tree(
                "1.0",
                CONTROL,
                &[("debian/patches/series", TestFile::new("a.patch\n"))],
            ),
            // Native 3.0 is not Quilt.
            tree(
                "3.0 (native)",
                CONTROL,
                &[("debian/patches/series", TestFile::new(""))],
            ),
            // Series reached through a symlink inside the tree.
            tree(
                "1.0",
                CONTROL,
                &[
                    (
                        "debian/patches/series",
                        TestFile::symlink("../quilt-series"),
                    ),
                    ("debian/quilt-series", TestFile::new("a.patch\n")),
                ],
            ),
        ];
        SourceTreeTester::new(QuiltSeriesButNoBuildDep, pass, fail).test_and_snapshot();
    }
}
