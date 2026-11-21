use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::bytes::Regex;
use serde::Deserialize;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct DuplicateFontFile;

declare_rule! {
    /// A font file in the Data files is also shipped by another package in the
    /// archive, as lintian's `fonts.json` records. Reports `also in (<packages>)`,
    /// the packages that ship a font of that basename, sorted, with `xfonts-tipa`
    /// counted as `tipa`, and the font's path. A package that is one of the
    /// shippers, and every udeb, is not reported.
    ///
    /// Only regular files and hardlinks whose basename looks like a font are
    /// checked (`.ttf`, `.otf`, `.pfb`, `.woff`, `.woff2`, `.eot`, optionally
    /// `.gz`). The lookup is case-insensitive, as lintian's `installed_by` is.
    /// Shipper names repeat as often as the archive lists them, as in lintian.
    ///
    /// Differs from lintian: the table is read at run time from the vendored
    /// lintian 2.144.0 `data/fonts.json`, and a non-UTF-8 basename never finds
    /// a shipper.
    DuplicateFontFile,
    code = "LN1088",
    tag = "duplicate-font-file",
    default_selected = true,
    experimental = false,
}

/// lintian 2.144.0 `data/fonts.json`: font basename to the packages that ship it.
#[derive(Deserialize)]
struct FontsFile {
    fonts: HashMap<String, FontEntry>,
}

#[derive(Deserialize)]
struct FontEntry {
    installed_by: Vec<String>,
}

static FONTS: LazyLock<HashMap<String, Vec<String>>> = LazyLock::new(|| {
    serde_json::from_str::<FontsFile>(include_str!("../../../data/fonts.json"))
        .expect("vendored fonts.json")
        .fonts
        .into_iter()
        .map(|(name, entry)| (name, entry.installed_by))
        .collect()
});

/// lintian's font test on a basename in `visit_installed_files`.
static FONT_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[\w-]+[.](?:[to]tf|pfb|woff2?|eot)(?:[.]gz)?$").expect("valid regex")
});

/// The basename of `path` when it is named like a font.
pub(super) fn font_basename(path: &Path) -> Option<&[u8]> {
    let name = path.file_name()?.as_bytes();
    FONT_NAME.is_match(name).then_some(name)
}

/// The packages lintian's `installed_by` returns for a font basename, with repeats.
pub(super) fn installed_by(basename: &[u8]) -> Vec<String> {
    std::str::from_utf8(basename)
        .ok()
        .and_then(|name| FONTS.get(&name.to_ascii_lowercase()))
        .cloned()
        .unwrap_or_default()
}

impl BinaryPackageRule for DuplicateFontFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let udeb = ctx.is_udeb();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let reports: Vec<(PathBuf, String)> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_file())
            .filter_map(|entry| {
                let shippers = installed_by(font_basename(entry.path())?);
                if shippers.is_empty() {
                    return None;
                }
                let mut renamed: Vec<String> = shippers
                    .into_iter()
                    .map(|name| {
                        if name == "xfonts-tipa" {
                            "tipa".to_string()
                        } else {
                            name
                        }
                    })
                    .collect();
                if udeb || renamed.contains(&package) {
                    return None;
                }
                renamed.sort();
                Some((
                    entry.path().to_path_buf(),
                    format!("also in ({})", renamed.join(" ")),
                ))
            })
            .collect();
        for (path, extra) in reports {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
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
            // Shipped by this package itself.
            font("usr/share/fonts/truetype/4shadow.ttf").package("fonts-aenigma"),
            // A font nobody else ships.
            font("usr/share/fonts/truetype/unknown-font.ttf"),
            // A udeb is not reported.
            font("usr/share/fonts/truetype/4shadow.ttf").udeb(),
            // Not a font name.
            font("usr/share/doc/example/4shadow.ttf.txt"),
            // A directory named like a font is not a file.
            BinaryPackageCase::from(vec![("usr/share/fonts/4shadow.ttf", TestFile::directory())]),
            // A gzipped font is looked up by its full basename, which no table lists.
            font("usr/share/fonts/truetype/4shadow.ttf.gz"),
            // xfonts-tipa ships tipa's fonts, so the tipa package is a shipper.
            font("usr/share/fonts/type1/tipa/tipa10.pfb").package("tipa"),
        ];
        let fail = vec![
            font("usr/share/fonts/truetype/4shadow.ttf"),
            // The basename is matched case-insensitively.
            font("usr/share/fonts/truetype/4SHADOW.TTF"),
            font("usr/share/fonts/type1/tipa/tipa10.pfb"),
            // The shipper list is sorted, and both shippers are named.
            font("usr/share/fonts/truetype/roboto-bold.ttf").package("example"),
            // A hardlink counts as a file.
            BinaryPackageCase::from(vec![
                (
                    "usr/share/fonts/truetype/unknown.ttf",
                    TestFile::new("font"),
                ),
                (
                    "usr/lib/example/4shadow.ttf",
                    TestFile::new("font").hardlink_to("usr/share/fonts/truetype/unknown.ttf"),
                ),
            ])
            .package("example"),
        ];
        BinaryPackageTester::new(DuplicateFontFile, pass, fail).test_and_snapshot();
    }
}
