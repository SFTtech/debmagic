use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::series_patches::{listed_patch, plain_series};

pub struct QuiltSeriesReferencesNonExistentPatch;

declare_rule! {
    /// A patch that `debian/patches/series` lists but that is not there.
    ///
    /// Runs when Format is `3.x (quilt)` (Dsc, else `debian/source/format`)
    /// or `Build-Depends-All` of Debian control satisfies `quilt`, and only
    /// when `debian/patches/series` is an open file (a regular file,
    /// hardlink, or a symlink that resolves to one inside the tree). Each
    /// listed name is resolved under `debian/patches/`, following symlinks
    /// inside the tree; it counts as present when that is a regular file or
    /// hardlink. Otherwise it is reported, once per listing, with the name
    /// as written (options dropped) as Extra and the series as Location (no
    /// line). Commented-out entries and vendor series do not count. Applies
    /// to the Source tree and Source package.
    QuiltSeriesReferencesNonExistentPatch,
    code = "LN1771",
    tag = "quilt-series-references-non-existent-patch",
    default_selected = true,
    experimental = false,
}

impl SourceRule for QuiltSeriesReferencesNonExistentPatch {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((series_path, names)) = plain_series(ctx) else {
            return;
        };
        let Some(files) = ctx.files() else {
            return;
        };
        let missing: Vec<&str> = names
            .iter()
            .map(String::as_str)
            .filter(|name| listed_patch(files, name).is_none())
            .collect();
        for name in missing {
            ctx.diagnostic(Diagnostic::error(name).with_location(Location {
                path: series_path.clone(),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>

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

    fn tree(
        format: Option<&str>,
        control: &str,
        extra: &[(&str, TestFile)],
    ) -> Vec<(String, TestFile)> {
        let mut files = vec![("debian/control".to_string(), TestFile::new(control))];
        if let Some(format) = format {
            files.push((
                "debian/source/format".to_string(),
                TestFile::new(format!("{format}\n")),
            ));
        }
        for (path, file) in extra {
            files.push(((*path).to_string(), file.clone()));
        }
        files
    }

    fn quilt(extra: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        tree(Some("3.0 (quilt)"), CONTROL, extra)
    }

    fn file(content: &str) -> TestFile {
        TestFile::new(content)
    }

    #[test]
    fn source() {
        let pass = vec![
            quilt(&[]),
            // Every listed name is there, with options and a `./` prefix.
            quilt(&[
                (
                    "debian/patches/series",
                    file("a.patch\nb.patch -p1\n./c.patch\n"),
                ),
                ("debian/patches/a.patch", file("diff")),
                ("debian/patches/b.patch", file("diff")),
                ("debian/patches/c.patch", file("diff")),
            ]),
            // Commented-out names, the names of a vendor series, and a
            // trailing comment are not listed.
            quilt(&[
                (
                    "debian/patches/series",
                    file("# gone.patch\n##  also-gone.patch\nlive.patch # gone2.patch\n"),
                ),
                ("debian/patches/live.patch", file("diff")),
                ("debian/patches/debian.series", file("vendor-gone.patch\n")),
            ]),
            // A listed name that is a hardlink or a symlink inside the tree.
            quilt(&[
                ("debian/patches/series", file("hard.patch\nlink.patch\n")),
                ("debian/patches/real.patch", file("diff")),
                (
                    "debian/patches/hard.patch",
                    TestFile::new("diff").hardlink_to("debian/patches/real.patch"),
                ),
                ("debian/patches/link.patch", TestFile::symlink("real.patch")),
                ("debian/patches/live.patch", file("diff")),
            ]),
            // A name that climbs out of the patch directory into the tree.
            quilt(&[
                ("debian/patches/series", file("../other.patch\n")),
                ("debian/other.patch", file("diff")),
            ]),
            // Empty series.
            quilt(&[("debian/patches/series", file(""))]),
            // Not quilt: Format 1.0 without a quilt build dependency.
            tree(
                Some("1.0"),
                CONTROL,
                &[("debian/patches/series", file("gone.patch\n"))],
            ),
            // A dangling series symlink is not open.
            quilt(&[("debian/patches/series", TestFile::symlink("missing"))]),
            // A series that is a directory is not open.
            quilt(&[("debian/patches/series", TestFile::directory())]),
        ];
        let fail = vec![
            // A missing name, a name that is a directory, and one that is a
            // dangling symlink.
            quilt(&[
                (
                    "debian/patches/series",
                    file("gone.patch -p1\nsub\ndangling.patch\n"),
                ),
                ("debian/patches/sub", TestFile::directory()),
                (
                    "debian/patches/dangling.patch",
                    TestFile::symlink("missing"),
                ),
            ]),
            // A symlink that escapes the tree, and one that resolves to a
            // directory.
            quilt(&[
                (
                    "debian/patches/series",
                    file("escape.patch\ndir-link.patch\n"),
                ),
                (
                    "debian/patches/escape.patch",
                    TestFile::symlink("../../../outside.patch"),
                ),
                ("debian/patches/dir-link.patch", TestFile::symlink("sub")),
                ("debian/patches/sub", TestFile::directory()),
            ]),
            // A name listed twice is reported twice.
            quilt(&[("debian/patches/series", file("gone.patch\ngone.patch\n"))]),
            // A name that climbs out of the patch directory and past the
            // tree root.
            quilt(&[("debian/patches/series", file("../../../gone.patch\n"))]),
            // Format 1.0 with a quilt build dependency is quilt.
            tree(
                Some("1.0"),
                CONTROL_QUILT,
                &[("debian/patches/series", file("gone.patch\n"))],
            ),
            // A series reached through a symlink inside the tree: the
            // Location is the series path as listed.
            tree(
                Some("3.0 (quilt)"),
                CONTROL,
                &[
                    (
                        "debian/patches/series",
                        TestFile::symlink("../quilt-series"),
                    ),
                    ("debian/quilt-series", file("gone.patch\n")),
                ],
            ),
        ];
        SourceTreeTester::new(QuiltSeriesReferencesNonExistentPatch, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let files = quilt(&[("debian/patches/series", file("gone.patch\n"))]);
        let pass = vec![
            SourcePackageCase::from(files.clone())
                .dsc(Dsc::parse("Format: 1.0\nVersion: 1.0-1\nSource: example\n").expect("dsc")),
        ];
        let fail = vec![SourcePackageCase::from(files).dsc(
            Dsc::parse("Format: 3.0 (quilt)\nVersion: 1.0-1\nSource: example\n").expect("dsc"),
        )];
        SourcePackageTester::new(QuiltSeriesReferencesNonExistentPatch, pass, fail)
            .test_and_snapshot();
    }
}
