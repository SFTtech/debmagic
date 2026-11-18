use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::ln0631_file_in_discouraged_x11_font_directory::x11_font_subdir;

pub struct FileInUnknownX11FontDirectory;

declare_rule! {
    /// A file is shipped in an unrecognised X11 font directory.
    ///
    /// `usr/share/fonts/X11/<dir>/<anything>` where `<dir>` is none of
    /// `100dpi`, `75dpi`, `misc`, `Type1`, `encodings`, `util`, or the
    /// discouraged `PEX`, `CID`, `Speedo`, `cyrillic`. Directories are
    /// skipped. Extra is empty. The path is the Location. Udebs are checked
    /// too.
    FileInUnknownX11FontDirectory,
    code = "LN0632",
    tag = "file-in-unknown-x11-font-directory",
    default_selected = true,
    experimental = false,
}

const KNOWN: &[&str] = &[
    "100dpi",
    "75dpi",
    "misc",
    "Type1",
    "encodings",
    "util",
    "PEX",
    "CID",
    "Speedo",
    "cyrillic",
];

impl BinaryPackageRule for FileInUnknownX11FontDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut paths = Vec::new();
        for entry in files.entries() {
            if matches!(entry.kind(), EntryKind::Directory) {
                continue;
            }
            let unknown = entry
                .path()
                .to_str()
                .and_then(x11_font_subdir)
                .is_some_and(|subdir| !KNOWN.contains(&subdir));
            if unknown {
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

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn font(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("font\n"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            font("usr/share/fonts/X11/misc/foo.pcf.gz"),
            font("usr/share/fonts/X11/Type1/foo"),
            font("usr/share/fonts/X11/PEX/foo"),
            font("usr/share/fonts/X11/other"),
            font("usr/share/fonts/truetype/other/foo"),
            BinaryPackageCase::from(vec![(
                "usr/share/fonts/X11/other/sub".to_string(),
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            font("usr/share/fonts/X11/other/foo.pcf.gz"),
            font("usr/share/fonts/X11/Misc/foo"),
            font("usr/share/fonts/X11/other/nested/foo"),
            font("usr/share/fonts/X11/other/foo").udeb(),
        ];
        BinaryPackageTester::new(FileInUnknownX11FontDirectory, pass, fail).test_and_snapshot();
    }
}
