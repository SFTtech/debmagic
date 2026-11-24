use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct RasterImageInScalableDirectory;

declare_rule! {
    /// A PNG or XPM image is installed into an icon theme's `scalable` directory.
    ///
    /// Walks the Installed files (Data files) of the Binary package, udebs
    /// included, and reports each regular file or hardlink whose path matches
    /// `/icons/<theme>/scalable/...` and ends in `.png` or `.xpm`. Only vector
    /// graphics belong in a `scalable` directory. Symlinks are not reported.
    /// Extra is empty; the installed path is the Location, with no line number.
    RasterImageInScalableDirectory,
    code = "LN1043",
    tag = "raster-image-in-scalable-directory",
    default_selected = true,
    experimental = false,
}

/// lintian's `visit_installed_files` name test, on raw path bytes.
static SCALABLE_RASTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u)/icons/[^/]+/scalable/.*\.(?:png|xpm)$").expect("valid regex")
});

impl BinaryPackageRule for RasterImageInScalableDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_file())
            .filter(|entry| SCALABLE_RASTER.is_match(entry.path().as_os_str().as_encoded_bytes()))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
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
            // Raster images in sized directories are fine.
            tree(&[(
                "usr/share/icons/hicolor/48x48/apps/foo.png",
                TestFile::new("png"),
            )]),
            // Vector graphics belong in `scalable`.
            tree(&[(
                "usr/share/icons/hicolor/scalable/apps/foo.svg",
                TestFile::new("svg"),
            )]),
            // Not an icon theme, or the `scalable` directory is not the theme's child.
            tree(&[("usr/share/pixmaps/scalable/foo.png", TestFile::new("png"))]),
            tree(&[(
                "usr/share/icons/hicolor/scalable/apps/foo.png/extra",
                TestFile::new("x"),
            )]),
            tree(&[(
                "usr/share/icons/hicolor/scalable/apps/foo.gif",
                TestFile::new("gif"),
            )]),
            // A symlink is not a file.
            tree(&[(
                "usr/share/icons/hicolor/scalable/apps/foo.png",
                TestFile::symlink("../48x48/apps/foo.png"),
            )]),
            // A directory named like an image is not a file.
            tree(&[(
                "usr/share/icons/hicolor/scalable/apps/foo.png",
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            tree(&[(
                "usr/share/icons/hicolor/scalable/apps/foo.png",
                TestFile::new("png"),
            )]),
            tree(&[(
                "usr/share/icons/hicolor/scalable/apps/foo.xpm",
                TestFile::new("xpm"),
            )]),
            // A hardlink is a file.
            tree(&[(
                "usr/share/icons/hicolor/scalable/apps/foo.png",
                TestFile::new("png").hardlink(),
            )]),
            // Any theme name, and nested subdirectories under `scalable`.
            tree(&[(
                "usr/share/icons/Adwaita/scalable/actions/deep/foo.png",
                TestFile::new("png"),
            )]),
            // Udebs are checked too.
            tree(&[(
                "usr/share/icons/hicolor/scalable/apps/foo.png",
                TestFile::new("png"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(RasterImageInScalableDirectory, pass, fail).test_and_snapshot();
    }
}
