use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsCompiledFontFile;

declare_rule! {
    /// A compiled X11 font index is shipped under a known font directory.
    ///
    /// Files in `usr/share/fonts/X11/{100dpi,75dpi,misc,Type1,encodings,util}`
    /// named `encodings.dir` or `fonts.dir`, `fonts.scale`, or `fonts.alias`.
    /// Directories are skipped. Extra is empty. The path is the Location.
    /// Udebs are checked too.
    PackageContainsCompiledFontFile,
    code = "LN0415",
    tag = "package-contains-compiled-font-file",
    default_selected = true,
    experimental = false,
}

const FONT_DIRS: &[&str] = &["100dpi", "75dpi", "misc", "Type1", "encodings", "util"];

impl BinaryPackageRule for PackageContainsCompiledFontFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut paths = Vec::new();
        for entry in files.entries() {
            if matches!(entry.kind(), EntryKind::Directory) {
                continue;
            }
            if entry.path().to_str().is_some_and(is_compiled_font) {
                paths.push(entry.path().to_path_buf());
            }
        }
        paths.sort();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn is_compiled_font(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("usr/share/fonts/X11/") else {
        return false;
    };
    let Some((subdir, file)) = rest.split_once('/') else {
        return false;
    };
    if file.is_empty() || !FONT_DIRS.contains(&subdir) {
        return false;
    }
    let Some(name) = Path::new(path).file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name == "encodings.dir" || matches!(name, "fonts.dir" | "fonts.scale" | "fonts.alias")
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn font(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("fonts\n"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            font("usr/share/fonts/X11/misc/fixed.pcf.gz"),
            font("usr/share/fonts/X11/PEX/fonts.dir"),
            font("usr/share/fonts/truetype/fonts.dir"),
            BinaryPackageCase::from(vec![(
                "usr/share/fonts/X11/misc".to_string(),
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            font("usr/share/fonts/X11/misc/fonts.dir"),
            font("usr/share/fonts/X11/misc/fonts.scale"),
            font("usr/share/fonts/X11/misc/fonts.alias"),
            font("usr/share/fonts/X11/misc/encodings.dir"),
            font("usr/share/fonts/X11/Type1/nested/fonts.dir"),
        ];
        BinaryPackageTester::new(PackageContainsCompiledFontFile, pass, fail).test_and_snapshot();
    }
}
