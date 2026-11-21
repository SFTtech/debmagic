use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::ln1088_duplicate_font_file::{font_basename, installed_by};

pub struct FontInNonFontPackage;

declare_rule! {
    /// A font file in the Data files that no archive package ships (per lintian's
    /// `fonts.json`, see `duplicate-font-file`) sits in a package whose name does
    /// not start with `ttf-`, `otf-`, `t1-`, `fonts-` or `xfonts-`. Reports the
    /// font's path with an empty extra. Font-named regular files and hardlinks
    /// are checked, as in `duplicate-font-file`.
    ///
    /// Skipped for fonts under `usr/lib/R/site-library/`, and for fonts that a
    /// package in the archive ships (those are `duplicate-font-file`'s business).
    /// Woff and woff2 files are not exempt here; `woff-font-in-wrong-dir` checks
    /// them too.
    ///
    /// Differs from lintian: the lint runs at package level and the table comes
    /// from the vendored lintian 2.144.0 `data/fonts.json`. Lintian's
    /// `font-outside-font-dir`, which is info-level and not a Rule, is not emitted.
    FontInNonFontPackage,
    code = "LN1089",
    tag = "font-in-non-font-package",
    default_selected = true,
    experimental = false,
}

/// Name prefixes of the packages that may ship fonts: lintian's `^(?:[ot]tf|t1|x?fonts)-`.
const FONT_PACKAGE_PREFIXES: [&str; 5] = ["ttf-", "otf-", "t1-", "fonts-", "xfonts-"];

/// lintian's `$item->name =~ m{^usr/lib/R/site-library/}`, skipped by the font Checks.
pub(super) fn in_r_site_library(path: &Path) -> bool {
    path.as_os_str()
        .as_bytes()
        .starts_with(b"usr/lib/R/site-library/")
}

impl BinaryPackageRule for FontInNonFontPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if FONT_PACKAGE_PREFIXES
            .iter()
            .any(|prefix| ctx.package_name().starts_with(prefix))
        {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_file())
            .filter(|entry| {
                font_basename(entry.path())
                    .is_some_and(|basename| installed_by(basename).is_empty())
                    && !in_r_site_library(entry.path())
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
            font("usr/share/fonts/truetype/unknown.ttf").package("fonts-example"),
            font("usr/share/fonts/truetype/unknown.ttf").package("ttf-example"),
            font("usr/share/fonts/truetype/unknown.ttf").package("otf-example"),
            font("usr/share/fonts/type1/unknown.pfb").package("t1-example"),
            font("usr/share/fonts/X11/unknown.pcf").package("xfonts-example"),
            // Shipped by the archive: duplicate-font-file reports it, not this Rule.
            font("usr/share/fonts/truetype/4shadow.ttf").package("example"),
            // Under the R site library.
            font("usr/lib/R/site-library/example/fonts/unknown.ttf").package("r-example"),
            // Not a font name.
            font("usr/share/doc/example/unknown.ttf.txt").package("example"),
            // A directory named like a font is not a file.
            BinaryPackageCase::from(vec![("usr/share/fonts/unknown.ttf", TestFile::directory())])
                .package("example"),
        ];
        let fail = vec![
            // The package name must start with the prefix, not contain it.
            font("usr/share/fonts/truetype/unknown.ttf").package("example-fonts-data"),
            // Only the path prefix counts, not a sibling directory name.
            font("usr/lib/R/site-library-other/unknown.ttf").package("example"),
            font("usr/share/fonts/truetype/unknown.ttf").package("example"),
            font("usr/share/fonts/truetype/unknown.otf").package("files-fonts"),
            // A font name is matched case-insensitively, a hardlink is a file.
            font("usr/share/fonts/truetype/UNKNOWN.TTF").package("example"),
            BinaryPackageCase::from(vec![
                (
                    "usr/share/fonts/truetype/unknown.ttf",
                    TestFile::new("font"),
                ),
                (
                    "usr/lib/example/unknown.ttf",
                    TestFile::new("font").hardlink_to("usr/share/fonts/truetype/unknown.ttf"),
                ),
            ])
            .package("example"),
            // A udeb is checked too.
            font("usr/share/fonts/truetype/unknown.ttf")
                .package("example")
                .udeb(),
            // A gzipped font is a font name.
            font("usr/share/fonts/truetype/unknown.ttf.gz").package("example"),
            // Woff fonts are also checked by this Rule.
            font("usr/share/fonts/woff/unknown.woff").package("example"),
        ];
        BinaryPackageTester::new(FontInNonFontPackage, pass, fail).test_and_snapshot();
    }
}
