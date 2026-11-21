use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsMultipleDpiFonts;

declare_rule! {
    /// The Binary package ships both 100dpi and 75dpi bitmap fonts.
    ///
    /// Collects the X11 bitmap font subdirectories that the package's Installed
    /// files (Data files, udebs included) use: a non-directory entry at
    /// `usr/share/fonts/X11/<subdir>/<something>` counts for `100dpi`, `75dpi`
    /// or `misc`, and any other subdirectory is ignored. Reports once when both
    /// `100dpi` and `75dpi` are present. Extra is empty and there is no Location,
    /// as lintian's package-level `hint`.
    ///
    /// The subdirectory test and the `misc` case are shared with
    /// `package-mixes-misc-and-dpi-fonts`.
    PackageContainsMultipleDpiFonts,
    code = "LN1044",
    tag = "package-contains-multiple-dpi-fonts",
    default_selected = true,
    experimental = false,
}

/// lintian's `visit_installed_files` font-directory test, on raw path bytes.
static X11_FONT_SUBDIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)^usr/share/fonts/X11/([^/]+)/\S+").expect("valid regex"));

/// The bitmap font subdirectories (`100dpi`, `75dpi`, `misc`) that hold at
/// least one non-directory Installed file. Shared with LN1045.
pub(super) fn bitmap_font_subdirs(files: &FileTree) -> BTreeSet<&'static str> {
    let mut found = BTreeSet::new();
    for entry in files.entries() {
        if entry.is_dir() {
            continue;
        }
        let Some(captures) = X11_FONT_SUBDIR.captures(entry.path().as_os_str().as_encoded_bytes())
        else {
            continue;
        };
        let subdir = &captures[1];
        for name in ["100dpi", "75dpi", "misc"] {
            if subdir == name.as_bytes() {
                found.insert(name);
            }
        }
    }
    found
}

impl BinaryPackageRule for PackageContainsMultipleDpiFonts {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let subdirs = bitmap_font_subdirs(files);
        if subdirs.contains("100dpi") && subdirs.contains("75dpi") {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(entries: &[(&str, TestFile)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            tree(&[]),
            // One resolution only.
            tree(&[("usr/share/fonts/X11/100dpi/foo.pcf.gz", TestFile::new("x"))]),
            tree(&[("usr/share/fonts/X11/75dpi/foo.pcf.gz", TestFile::new("x"))]),
            // Both directories exist but hold no file.
            tree(&[
                ("usr/share/fonts/X11/100dpi", TestFile::directory()),
                ("usr/share/fonts/X11/75dpi", TestFile::directory()),
            ]),
            // A directory entry below the font directory does not count.
            tree(&[
                ("usr/share/fonts/X11/100dpi/sub", TestFile::directory()),
                ("usr/share/fonts/X11/75dpi/sub", TestFile::directory()),
            ]),
            // A bare file named like the directory is not inside it.
            tree(&[
                ("usr/share/fonts/X11/100dpi", TestFile::new("x")),
                ("usr/share/fonts/X11/75dpi", TestFile::new("x")),
            ]),
            // The path must start at `usr/share/fonts/X11`.
            tree(&[
                (
                    "opt/usr/share/fonts/X11/100dpi/foo.pcf.gz",
                    TestFile::new("x"),
                ),
                ("usr/share/fonts/Y11/75dpi/foo.pcf.gz", TestFile::new("x")),
            ]),
            // Other subdirectories do not count.
            tree(&[
                (
                    "usr/share/fonts/X11/100dpi-extra/foo.pcf.gz",
                    TestFile::new("x"),
                ),
                ("usr/share/fonts/X11/75dpi/foo.pcf.gz", TestFile::new("x")),
            ]),
        ];
        let fail = vec![
            tree(&[
                ("usr/share/fonts/X11/100dpi/foo.pcf.gz", TestFile::new("x")),
                ("usr/share/fonts/X11/75dpi/foo.pcf.gz", TestFile::new("x")),
            ]),
            // Symlinks are non-directory entries and count.
            tree(&[
                (
                    "usr/share/fonts/X11/100dpi/foo.pcf.gz",
                    TestFile::symlink("x"),
                ),
                ("usr/share/fonts/X11/75dpi/foo.pcf.gz", TestFile::new("x")),
            ]),
            // Hardlinks count.
            tree(&[
                (
                    "usr/share/fonts/X11/100dpi/foo.pcf.gz",
                    TestFile::new("x").hardlink(),
                ),
                ("usr/share/fonts/X11/75dpi/foo.pcf.gz", TestFile::new("x")),
            ]),
            // Udebs are checked too.
            tree(&[
                ("usr/share/fonts/X11/100dpi/foo.pcf.gz", TestFile::new("x")),
                ("usr/share/fonts/X11/75dpi/foo.pcf.gz", TestFile::new("x")),
            ])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsMultipleDpiFonts, pass, fail).test_and_snapshot();
    }
}
