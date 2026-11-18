use std::collections::HashSet;
use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

use super::ln0396_format_3_0_but_debian_changes_patch::{is_quilt_format, source_format};

pub struct PatchFilePresentButNotMentionedInSeries;

declare_rule! {
    /// A file under `debian/patches/` that no Quilt series mentions.
    ///
    /// Runs when Format is `3.x (quilt)` (Dsc, else `debian/source/format`)
    /// or `Build-Depends-All` of Debian control satisfies `quilt`, and not
    /// for Format `2.0`. Every Quilt series directly in `debian/patches/`
    /// (`series`, `<vendor>.series`) counts: its own name, and the first word
    /// of each line, commented out or not. Every Entry below
    /// `debian/patches/` except directories is checked by its path relative
    /// to `debian/patches/`. Basenames `README`, `README.patches`, and any
    /// basename containing `.in` are skipped. Extra is empty. The Location is
    /// the file. Applies to the Source tree and Source package.
    PatchFilePresentButNotMentionedInSeries,
    code = "LN0706",
    tag = "patch-file-present-but-not-mentioned-in-series",
    default_selected = true,
    experimental = false,
}

/// Lintian's `Build-Depends-All`.
pub(super) const BUILD_DEPENDS_ALL: &[&str] =
    &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

impl SourceRule for PatchFilePresentButNotMentionedInSeries {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let format = source_format(ctx);
        let uses_quilt = is_quilt_format(&format)
            || ctx.debian_control().is_some_and(|control| {
                control
                    .source()
                    .relations_and(BUILD_DEPENDS_ALL)
                    .satisfies("quilt")
            });
        if !uses_quilt || format == "2.0" {
            return;
        }
        let mut known: HashSet<String> = HashSet::new();
        for (path, series) in ctx.quilt_series() {
            known.insert(basename(&path));
            known.extend(series.mentions().map(str::to_string));
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let patch_dir = Path::new("debian/patches");
        let unmentioned: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter_map(|entry| {
                let name = entry.path().strip_prefix(patch_dir).ok()?;
                if name.as_os_str().is_empty() {
                    return None;
                }
                let base = basename(entry.path());
                if base == "README" || base == "README.patches" || base.contains(".in") {
                    return None;
                }
                if known.contains(name.to_string_lossy().as_ref()) {
                    return None;
                }
                Some(entry.path().to_path_buf())
            })
            .collect();
        for path in unmentioned {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn basename(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
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
    fn test() {
        let pass = vec![
            quilt(&[]),
            quilt(&[
                ("debian/patches/series", file("a.patch\nb.patch -p0\n")),
                ("debian/patches/a.patch", file("diff")),
                ("debian/patches/b.patch", file("diff")),
            ]),
            // Commented out, in a vendor series, in a subdirectory, and the
            // ignore list.
            quilt(&[
                (
                    "debian/patches/series",
                    file("# disabled.patch\n##  also-disabled.patch\nsub/c.patch\n"),
                ),
                ("debian/patches/debian.series", file("vendor.patch\n")),
                ("debian/patches/.series", file("dot.patch\n")),
                ("debian/patches/disabled.patch", file("diff")),
                ("debian/patches/also-disabled.patch", file("diff")),
                ("debian/patches/vendor.patch", file("diff")),
                ("debian/patches/dot.patch", file("diff")),
                ("debian/patches/sub", TestFile::directory()),
                ("debian/patches/sub/c.patch", file("diff")),
                ("debian/patches/README", file("readme")),
                ("debian/patches/README.patches", file("readme")),
                ("debian/patches/sub/README", file("readme")),
                ("debian/patches/series.in", file("template")),
                ("debian/patches/x.install.patch", file("diff")),
            ]),
            // Not quilt: Format 1.0 without a quilt build dependency.
            tree(
                Some("1.0"),
                CONTROL,
                &[("debian/patches/stray.patch", file("diff"))],
            ),
            // Format 2.0 is skipped even with a quilt build dependency.
            tree(
                Some("2.0"),
                CONTROL_QUILT,
                &[("debian/patches/stray.patch", file("diff"))],
            ),
            // A series symlink to a file in the tree counts.
            quilt(&[
                (
                    "debian/patches/series",
                    TestFile::symlink("../quilt-series"),
                ),
                ("debian/quilt-series", file("a.patch\n")),
                ("debian/patches/a.patch", file("diff")),
            ]),
            // A series under a subdirectory does not count as a series.
            quilt(&[
                ("debian/patches/series", file("sub/series\nsub/d.patch\n")),
                ("debian/patches/sub/series", file("other.patch\n")),
                ("debian/patches/sub/d.patch", file("diff")),
            ]),
        ];
        let fail = vec![
            quilt(&[
                ("debian/patches/series", file("# some words\nused.patch\n")),
                ("debian/patches/used.patch", file("diff")),
                ("debian/patches/unused.patch", file("diff")),
                ("debian/patches/sub/unused-2.patch", file("diff")),
                ("debian/patches/link.patch", TestFile::symlink("used.patch")),
            ]),
            // No series at all, Format missing but quilt build dependency.
            tree(
                None,
                CONTROL_QUILT,
                &[("debian/patches/only.patch", file("diff"))],
            ),
            // A dangling series symlink does not count, and is itself
            // reported.
            quilt(&[
                ("debian/patches/series", TestFile::symlink("missing")),
                ("debian/patches/a.patch", file("diff")),
            ]),
            // Options are a second word, never a mention; the name must match
            // exactly.
            quilt(&[
                ("debian/patches/series", file("a.patch -p1\n./b.patch\n")),
                ("debian/patches/a.patch", file("diff")),
                ("debian/patches/b.patch", file("diff")),
                ("debian/patches/-p1", file("diff")),
            ]),
        ];
        SourceTreeTester::new(PatchFilePresentButNotMentionedInSeries, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let files = tree(
            Some("1.0"),
            CONTROL,
            &[("debian/patches/stray.patch", file("diff"))],
        );
        let pass = vec![SourcePackageCase::from(files.clone())];
        let fail = vec![SourcePackageCase::from(files).dsc(
            Dsc::parse("Format: 3.0 (quilt)\nVersion: 1.0-1\nSource: example\n").expect("dsc"),
        )];
        SourcePackageTester::new(PatchFilePresentButNotMentionedInSeries, pass, fail)
            .test_and_snapshot();
    }
}
