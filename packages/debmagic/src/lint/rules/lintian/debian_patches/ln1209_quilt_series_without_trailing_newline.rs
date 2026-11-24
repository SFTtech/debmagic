use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::ln0396_format_3_0_but_debian_changes_patch::{is_quilt_format, source_format};
use super::ln0706_patch_file_present_but_not_mentioned_in_series::BUILD_DEPENDS_ALL;

pub struct QuiltSeriesWithoutTrailingNewline;

declare_rule! {
    /// A `debian/patches/series` whose last line has no trailing newline.
    ///
    /// Runs when Format is `3.x (quilt)` (Dsc, else `debian/source/format`)
    /// or `Build-Depends-All` of Debian control satisfies `quilt`, and only
    /// when `debian/patches/series` is an open file (a regular file,
    /// hardlink, or a symlink that resolves to one inside the tree). An empty
    /// file is fine. Extra is empty. The Location is the series file, with no
    /// line. Applies to the Source tree and Source package.
    QuiltSeriesWithoutTrailingNewline,
    code = "LN1209",
    tag = "quilt-series-without-trailing-newline",
    default_selected = true,
    experimental = false,
}

impl SourceRule for QuiltSeriesWithoutTrailingNewline {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let quilt_format = is_quilt_format(&source_format(ctx));
        let builds_with_quilt = ctx.debian_control().is_some_and(|control| {
            control
                .source()
                .relations_and(BUILD_DEPENDS_ALL)
                .satisfies("quilt")
        });
        if !quilt_format && !builds_with_quilt {
            return;
        }
        let series_path = Path::new("debian/patches/series");
        let Some((path, series)) = ctx
            .quilt_series()
            .into_iter()
            .find(|(path, _)| path == series_path)
        else {
            return;
        };
        if series.has_unterminated_line() {
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
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const CONTROL_QUILT: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>
Build-Depends: debhelper-compat (= 13), quilt

Package: example
Architecture: all
Description: example
 extra
";

    const CONTROL: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>
Build-Depends: debhelper-compat (= 13)

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
            // Every line ends with a newline.
            tree(
                "3.0 (quilt)",
                CONTROL,
                &[(
                    "debian/patches/series",
                    TestFile::new("a.patch\nb.patch -p1\n"),
                )],
            ),
            // Empty series.
            tree(
                "3.0 (quilt)",
                CONTROL,
                &[("debian/patches/series", TestFile::new(""))],
            ),
            // Not checked without quilt format or build dependency.
            tree(
                "1.0",
                CONTROL,
                &[("debian/patches/series", TestFile::new("a.patch"))],
            ),
            // Vendor series is not the plain series.
            tree(
                "3.0 (quilt)",
                CONTROL,
                &[("debian/patches/vendor.series", TestFile::new("a.patch"))],
            ),
        ];
        let fail = vec![
            // Last line lacks its newline.
            tree(
                "3.0 (quilt)",
                CONTROL,
                &[("debian/patches/series", TestFile::new("a.patch\nb.patch"))],
            ),
            // A comment as the last line without newline.
            tree(
                "1.0",
                CONTROL_QUILT,
                &[("debian/patches/series", TestFile::new("a.patch\n# end"))],
            ),
            // Series reached through a symlink inside the tree.
            tree(
                "3.0 (quilt)",
                CONTROL,
                &[
                    (
                        "debian/patches/series",
                        TestFile::symlink("../quilt-series"),
                    ),
                    ("debian/quilt-series", TestFile::new("a.patch")),
                ],
            ),
        ];
        SourceTreeTester::new(QuiltSeriesWithoutTrailingNewline, pass, fail).test_and_snapshot();
    }
}
