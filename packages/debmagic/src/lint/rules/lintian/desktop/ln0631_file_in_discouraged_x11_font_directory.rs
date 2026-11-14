use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct FileInDiscouragedX11FontDirectory;

declare_rule! {
    /// A file is shipped in a discouraged X11 font directory.
    ///
    /// `usr/share/fonts/X11/{PEX,CID,Speedo,cyrillic}/<anything>`. Directories
    /// are skipped; symlinks and nested files count. Extra is empty. The path
    /// is the Location. Udebs are checked too.
    FileInDiscouragedX11FontDirectory,
    code = "LN0631",
    tag = "file-in-discouraged-x11-font-directory",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for FileInDiscouragedX11FontDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut paths = Vec::new();
        for entry in files.entries() {
            if matches!(entry.kind(), EntryKind::Directory) {
                continue;
            }
            let discouraged = entry
                .path()
                .to_str()
                .and_then(x11_font_subdir)
                .is_some_and(|subdir| DISCOURAGED.contains(&subdir));
            if discouraged {
                paths.push(entry.path().to_path_buf());
            }
        }
        paths.sort();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

const DISCOURAGED: &[&str] = &["PEX", "CID", "Speedo", "cyrillic"];

/// `usr/share/fonts/X11/([^/]+)/\S+`: the directory below `X11`.
pub(super) fn x11_font_subdir(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("usr/share/fonts/X11/")?;
    let (subdir, file) = rest.split_once('/')?;
    let first = file.chars().next()?;
    (!subdir.is_empty() && !first.is_whitespace()).then_some(subdir)
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
            font("usr/share/fonts/X11/other/foo.pcf.gz"),
            font("usr/share/fonts/X11/PEX"),
            font("usr/share/fonts/truetype/PEX/foo"),
            font("usr/share/fonts/X11/PEX/ foo"),
            BinaryPackageCase::from(vec![(
                "usr/share/fonts/X11/PEX/sub".to_string(),
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            font("usr/share/fonts/X11/PEX/foo.pcf.gz"),
            font("usr/share/fonts/X11/CID/foo.pcf.gz"),
            font("usr/share/fonts/X11/Speedo/foo.pcf.gz"),
            font("usr/share/fonts/X11/cyrillic/nested/foo.pcf.gz"),
            BinaryPackageCase::from(vec![(
                "usr/share/fonts/X11/CID/link".to_string(),
                TestFile::symlink("elsewhere"),
            )]),
            font("usr/share/fonts/X11/PEX/foo").udeb(),
        ];
        BinaryPackageTester::new(FileInDiscouragedX11FontDirectory, pass, fail).test_and_snapshot();
    }
}
