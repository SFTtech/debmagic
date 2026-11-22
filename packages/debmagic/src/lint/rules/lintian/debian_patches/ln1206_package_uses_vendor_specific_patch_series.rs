use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::ln0396_format_3_0_but_debian_changes_patch::source_format;

pub struct PackageUsesVendorSpecificPatchSeries;

declare_rule! {
    /// A vendor-specific Quilt series file `debian/patches/<vendor>.series`.
    ///
    /// Skipped when Format is `2.0` (Dsc, else `debian/source/format`). Only
    /// series files directly in `debian/patches/` whose name ends in
    /// `.series` are reported; the plain `series` is fine. A series counts
    /// when it is a regular file, hardlink, or a symlink that resolves to one
    /// inside the tree; directories and dangling or escaping symlinks are
    /// skipped. Extra is empty. The Location is the series file itself, not
    /// the target of a symlink. Applies to the Source tree and Source package.
    PackageUsesVendorSpecificPatchSeries,
    code = "LN1206",
    tag = "package-uses-vendor-specific-patch-series",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PackageUsesVendorSpecificPatchSeries {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if source_format(ctx) == "2.0" {
            return;
        }
        for (path, _) in ctx.quilt_series() {
            if path.extension().is_some_and(|ext| ext == "series") {
                ctx.diagnostic(Diagnostic::error("").with_location(Location {
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
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(format: &str, extra: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        let mut files = vec![(
            "debian/source/format".to_string(),
            TestFile::new(format!("{format}\n")),
        )];
        for (path, file) in extra {
            files.push(((*path).to_string(), file.clone()));
        }
        files
    }

    #[test]
    fn source() {
        let pass = vec![
            // Only the plain series file.
            tree(
                "1.0",
                &[("debian/patches/series", TestFile::new("a.patch\n"))],
            ),
            // Format 2.0 is skipped.
            tree(
                "2.0",
                &[("debian/patches/vendor.series", TestFile::new("a.patch\n"))],
            ),
            // Not directly in debian/patches.
            tree(
                "1.0",
                &[(
                    "debian/patches/sub/vendor.series",
                    TestFile::new("a.patch\n"),
                )],
            ),
            // A directory named like a vendor series is not a series.
            tree(
                "1.0",
                &[("debian/patches/vendor.series", TestFile::directory())],
            ),
            // A dangling symlink is not open.
            tree(
                "1.0",
                &[("debian/patches/vendor.series", TestFile::symlink("missing"))],
            ),
        ];
        let fail = vec![
            // Vendor series as a regular file.
            tree(
                "1.0",
                &[("debian/patches/vendor.series", TestFile::new("a.patch\n"))],
            ),
            // Quilt format as well.
            tree(
                "3.0 (quilt)",
                &[("debian/patches/vendor.series", TestFile::new("a.patch\n"))],
            ),
            // Symlink to a series inside the tree is reported at the link.
            tree(
                "1.0",
                &[
                    (
                        "debian/patches/ubuntu.series",
                        TestFile::symlink("../vendor-series"),
                    ),
                    ("debian/vendor-series", TestFile::new("a.patch\n")),
                ],
            ),
        ];
        SourceTreeTester::new(PackageUsesVendorSpecificPatchSeries, pass, fail).test_and_snapshot();
    }
}
