use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::ln0396_format_3_0_but_debian_changes_patch::source_format;
use super::ln0706_patch_file_present_but_not_mentioned_in_series::BUILD_DEPENDS_ALL;

pub struct QuiltBuildDepButNoSeriesFile;

declare_rule! {
    /// A non-Quilt source package that build-depends on `quilt` but has no
    /// `debian/patches/series` to list its patches.
    ///
    /// Skipped when Format is `3.x (quilt)` (Dsc, else `debian/source/format`),
    /// and when `Build-Depends-All` of Debian control does not satisfy
    /// `quilt`. The series counts when it is a regular file, hardlink, or a
    /// symlink that resolves to one inside the tree. Extra is empty. There is
    /// no Location: the Subject is the location. Applies to the Source tree
    /// and Source package.
    QuiltBuildDepButNoSeriesFile,
    code = "LN0892",
    tag = "quilt-build-dep-but-no-series-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for QuiltBuildDepButNoSeriesFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if super::ln0396_format_3_0_but_debian_changes_patch::is_quilt_format(&source_format(ctx)) {
            return;
        }
        let builds_with_quilt = ctx.debian_control().is_some_and(|control| {
            control
                .source()
                .relations_and(BUILD_DEPENDS_ALL)
                .satisfies("quilt")
        });
        if !builds_with_quilt {
            return;
        }
        let has_series = ctx
            .quilt_series()
            .iter()
            .any(|(path, _)| path == Path::new("debian/patches/series"));
        if !has_series {
            ctx.diagnostic(Diagnostic::warn(""));
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
            // Quilt format needs no series from the build dependency.
            tree("3.0 (quilt)", CONTROL, &[]),
            // No quilt build dependency.
            tree("1.0", CONTROL_PLAIN, &[]),
            // Series present as a regular file.
            tree(
                "1.0",
                CONTROL,
                &[("debian/patches/series", TestFile::new("a.patch\n"))],
            ),
            // Series present as a symlink to a file inside the tree.
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
        let fail = vec![
            // Format 1.0 with quilt build dependency and no series.
            tree("1.0", CONTROL, &[]),
            // Native 3.0 is not Quilt either.
            tree("3.0 (native)", CONTROL, &[]),
            // Series is a directory.
            tree(
                "1.0",
                CONTROL,
                &[("debian/patches/series", TestFile::directory())],
            ),
            // Series is a dangling symlink.
            tree(
                "1.0",
                CONTROL,
                &[("debian/patches/series", TestFile::symlink("missing"))],
            ),
        ];
        SourceTreeTester::new(QuiltBuildDepButNoSeriesFile, pass, fail).test_and_snapshot();
    }
}
