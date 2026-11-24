use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::ln0396_format_3_0_but_debian_changes_patch::{is_quilt_format, source_format};
use super::ln0706_patch_file_present_but_not_mentioned_in_series::BUILD_DEPENDS_ALL;

pub struct QuiltPatchWithNonStandardOptions;

declare_rule! {
    /// A line of `debian/patches/series` that gives a patch options other
    /// than `-p1`, such as `some-patch -p0`.
    ///
    /// Runs when Format is `3.x (quilt)` (Dsc, else `debian/source/format`)
    /// or `Build-Depends-All` of Debian control satisfies `quilt`, and only
    /// when `debian/patches/series` is an open file (a regular file,
    /// hardlink, or a symlink that resolves to one inside the tree). Comments
    /// and blank lines are ignored; a line whose options are exactly `-p1`
    /// is fine. Extra is the patch name and its options joined by one space
    /// (lintian keeps the original spacing; we do not). The Location is the
    /// series file and the 1-based line of the patch. Applies to the Source
    /// tree and Source package.
    QuiltPatchWithNonStandardOptions,
    code = "LN1207",
    tag = "quilt-patch-with-non-standard-options",
    default_selected = true,
    experimental = false,
}

impl SourceRule for QuiltPatchWithNonStandardOptions {
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
        for patch in series.patches() {
            let Some(options) = patch.options() else {
                continue;
            };
            if options == "-p1" {
                continue;
            }
            ctx.diagnostic(
                Diagnostic::warn(format!("{} {}", patch.name(), options)).with_location(Location {
                    path: path.clone(),
                    line: Some(patch.line()),
                    column: None,
                }),
            );
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

    const CONTROL_PLAIN: &str = "\
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
            // Standard -p1 and plain names only.
            tree(
                "3.0 (quilt)",
                CONTROL_QUILT,
                &[(
                    "debian/patches/series",
                    TestFile::new("# comment\n\na.patch\nb.patch -p1\n"),
                )],
            ),
            // No quilt format and no quilt build dependency: not checked.
            tree(
                "1.0",
                CONTROL_PLAIN,
                &[("debian/patches/series", TestFile::new("a.patch -p0\n"))],
            ),
            // Options on a vendor series are not this Tag's business.
            tree(
                "3.0 (quilt)",
                CONTROL_QUILT,
                &[(
                    "debian/patches/vendor.series",
                    TestFile::new("a.patch -p0\n"),
                )],
            ),
            // Series is a dangling symlink.
            tree(
                "3.0 (quilt)",
                CONTROL_QUILT,
                &[("debian/patches/series", TestFile::symlink("missing"))],
            ),
        ];
        let fail = vec![
            // Quilt format with -p0.
            tree(
                "3.0 (quilt)",
                CONTROL_QUILT,
                &[(
                    "debian/patches/series",
                    TestFile::new("a.patch\nsome-other-file -p0\n"),
                )],
            ),
            // Quilt build dependency on format 1.0, with several options.
            tree(
                "1.0",
                CONTROL_QUILT,
                &[("debian/patches/series", TestFile::new("a.patch -p1 -R\n"))],
            ),
            // Series reached through a symlink inside the tree.
            tree(
                "3.0 (quilt)",
                CONTROL_QUILT,
                &[
                    (
                        "debian/patches/series",
                        TestFile::symlink("../quilt-series"),
                    ),
                    ("debian/quilt-series", TestFile::new("a.patch -p0\n")),
                ],
            ),
        ];
        SourceTreeTester::new(QuiltPatchWithNonStandardOptions, pass, fail).test_and_snapshot();
    }
}
