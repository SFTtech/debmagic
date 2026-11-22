use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::ln1088_duplicate_font_file::{font_basename, installed_by};
use super::ln1089_font_in_non_font_package::in_r_site_library;

pub struct FontOutsideFontDir;

declare_rule! {
    /// A TrueType, OpenType, Type 1 or EOT font that no archive package ships
    /// (per lintian's `fonts.json`, see `duplicate-font-file`) is not under
    /// `usr/share/fonts/`. Reported at info severity, with an empty extra and the
    /// font's path as the Location. Woff and woff2 fonts are left to
    /// `woff-font-in-wrong-dir`. Font-named regular files and hardlinks are
    /// checked, as in `font-in-non-font-package`.
    ///
    /// Skipped for fonts under `usr/lib/R/site-library/` and for fonts that a
    /// package in the archive ships.
    ///
    /// Differs from lintian: the table comes from the vendored lintian 2.144.0
    /// `data/fonts.json`, and the lint runs at Binary package level, so the
    /// per-package `duplicate-font-file` check is not repeated here.
    FontOutsideFontDir,
    code = "LN1392",
    tag = "font-outside-font-dir",
    default_selected = false,
    experimental = false,
}

/// lintian's woff test on a basename in `visit_installed_files`.
static WOFF_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[\w-]+[.]woff2?(?:[.]gz)?$").expect("valid regex"));

impl BinaryPackageRule for FontOutsideFontDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_file())
            .filter(|entry| {
                font_basename(entry.path()).is_some_and(|basename| {
                    !WOFF_NAME.is_match(basename)
                        && installed_by(basename).is_empty()
                        && !in_r_site_library(entry.path())
                        && !entry
                            .path()
                            .as_os_str()
                            .as_bytes()
                            .starts_with(b"usr/share/fonts/")
                })
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn font(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new("font"))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            font("usr/share/fonts/truetype/unknown.ttf").package("example"),
            font("usr/share/fonts/type1/unknown.pfb").package("example"),
            // Woff fonts are woff-font-in-wrong-dir's business.
            font("usr/share/example/unknown.woff").package("example"),
            font("usr/share/example/unknown.woff2.gz").package("example"),
            // Shipped by the archive: duplicate-font-file reports it.
            font("usr/lib/example/4shadow.ttf").package("example"),
            // Under the R site library.
            font("usr/lib/R/site-library/example/fonts/unknown.ttf").package("r-example"),
            // Not a font name.
            font("usr/share/doc/example/unknown.ttf.txt").package("example"),
            // A directory named like a font is not a file.
            BinaryPackageCase::from(vec![(
                "usr/share/example/unknown.ttf",
                TestFile::directory(),
            )])
            .package("example"),
        ];
        let fail = vec![
            font("usr/share/example/unknown.ttf").package("example"),
            font("usr/lib/example/unknown.otf").package("example"),
            font("usr/lib/example/unknown.pfb").package("example"),
            font("usr/lib/example/unknown.eot").package("example"),
            // Only the path prefix counts, not a sibling directory name.
            font("usr/share/fontsextra/unknown.ttf").package("example"),
            // A font name is matched case-insensitively, a gzipped font too.
            font("usr/share/example/UNKNOWN.TTF.GZ").package("example"),
            // A hardlink is a file.
            BinaryPackageCase::from(vec![
                ("usr/share/fonts/unknown.ttf", TestFile::new("font")),
                (
                    "usr/lib/example/unknown.ttf",
                    TestFile::new("font").hardlink_to("usr/share/fonts/unknown.ttf"),
                ),
            ])
            .package("example"),
            // A udeb is checked too.
            font("usr/lib/example/unknown.ttf")
                .package("example")
                .udeb(),
        ];
        BinaryPackageTester::new(FontOutsideFontDir, pass, fail).test_and_snapshot();
    }
}
