use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::ln1088_duplicate_font_file::{font_basename, installed_by};
use super::ln1089_font_in_non_font_package::in_r_site_library;

pub struct WoffFontInWrongDir;

declare_rule! {
    /// A woff or woff2 font that no archive package ships (per lintian's
    /// `fonts.json`, see `duplicate-font-file`) is not under
    /// `usr/share/fonts-<name>/woff/` or `usr/share/fonts-<name>/woff2/`, the
    /// directories the Debian font packaging policy asks for. Reports the font's
    /// path with an empty extra. Font-named regular files and hardlinks are
    /// checked, `.gz` included.
    ///
    /// Skipped for fonts under `usr/lib/R/site-library/` and for fonts that a
    /// package in the archive ships.
    ///
    /// Differs from lintian: the directory test is a byte regex over the full
    /// path (lintian's `^usr/share/fonts-(.+)/woff2?/`), and the table comes from
    /// the vendored lintian 2.144.0 `data/fonts.json`.
    WoffFontInWrongDir,
    code = "LN1090",
    tag = "woff-font-in-wrong-dir",
    default_selected = true,
    experimental = false,
}

/// lintian's woff test on the basename of a font.
static WOFF_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[\w-]+[.]woff2?(?:[.]gz)?$").expect("valid regex"));

/// Whether a font basename is a woff or woff2 font.
fn is_woff(basename: &[u8]) -> bool {
    WOFF_NAME.is_match(basename)
}

/// lintian's `^usr/share/fonts-(.+)/woff2?/` on the path of a font.
static WOFF_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s-u)^usr/share/fonts-.+/woff2?/").expect("valid regex"));

impl BinaryPackageRule for WoffFontInWrongDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_file())
            .filter(|entry| {
                font_basename(entry.path())
                    .is_some_and(|basename| is_woff(basename) && installed_by(basename).is_empty())
                    && !in_r_site_library(entry.path())
                    && !WOFF_DIR.is_match(entry.path().as_os_str().as_encoded_bytes())
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
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
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn font(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new("font"))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            font("usr/share/fonts-example/woff/unknown.woff"),
            font("usr/share/fonts-example/woff2/unknown.woff2"),
            font("usr/share/fonts-example/woff/unknown.woff.gz"),
            // A nested font-family directory still counts.
            font("usr/share/fonts-example/family/woff/unknown.woff"),
            // Not woff: another font type is not this Rule's business.
            font("usr/share/fonts/truetype/unknown.ttf"),
            // Under the R site library.
            font("usr/lib/R/site-library/example/unknown.woff"),
            // Not a font name.
            font("usr/share/doc/example/unknown.woff.txt"),
            // A directory named like a font is not a file.
            BinaryPackageCase::from(vec![(
                "usr/share/fonts/unknown.woff",
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            font("usr/share/fonts/woff/unknown.woff"),
            font("usr/share/fonts/woff2/unknown.woff2"),
            font("usr/share/fonts-example/unknown.woff"),
            font("usr/share/fonts-example/truetype/unknown.woff2"),
            font("usr/share/fonts/woff/unknown.woff.gz"),
            // The directory name is matched case-sensitively, as lintian does.
            font("usr/share/Fonts-example/woff/unknown.woff"),
            // A hardlink counts as a file.
            BinaryPackageCase::from(vec![
                ("usr/share/fonts/woff/unknown.woff", TestFile::new("font")),
                (
                    "usr/lib/example/unknown.woff",
                    TestFile::new("font").hardlink_to("usr/share/fonts/woff/unknown.woff"),
                ),
            ]),
        ];
        BinaryPackageTester::new(WoffFontInWrongDir, pass, fail).test_and_snapshot();
    }
}
