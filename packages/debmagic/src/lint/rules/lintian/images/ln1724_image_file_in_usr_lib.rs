use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Entry;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct ImageFileInUsrLib;

declare_rule! {
    /// A bitmap or pixmap is installed under `usr/lib/`.
    ///
    /// Walks Data files. An entry under `usr/lib/` whose name ends in `.bmp`,
    /// `.gif`, `.jpg`, `.jpeg`, `.png`, `.tiff`, `.xbm`, `.xpm`, or `.xpb` is
    /// reported, unless it is a symlink or hardlink. Directories with such a
    /// name are reported too, as lintian does. The name suffix is matched
    /// case-sensitively. Udebs are checked too. Extra is empty; the Location is
    /// the entry.
    ImageFileInUsrLib,
    code = "LN1724",
    tag = "image-file-in-usr-lib",
    default_selected = false,
    experimental = false,
}

static IMAGE_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.(?:bmp|gif|jpe?g|png|tiff|x[pb]m)$").expect("valid regex"));

impl BinaryPackageRule for ImageFileInUsrLib {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| is_image_in_usr_lib(entry))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

fn is_image_in_usr_lib(entry: &Entry) -> bool {
    let Some(path) = entry.path().to_str() else {
        return false;
    };
    path.starts_with("usr/lib/")
        && IMAGE_SUFFIX.is_match(path)
        && entry.link_target().is_none()
        && entry.hardlink_target().is_none()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(vec![]),
            // Architecture-independent images belong in usr/share.
            case(vec![(
                "usr/share/foo/bar.png".to_string(),
                TestFile::new("x"),
            )]),
            // Not an image suffix.
            case(vec![(
                "usr/lib/foo/bar.txt".to_string(),
                TestFile::new("x"),
            )]),
            case(vec![(
                "usr/lib/foo/bar.PNG".to_string(),
                TestFile::new("x"),
            )]),
            // Not under usr/lib/ (a similar prefix does not count).
            case(vec![(
                "usr/library/bar.png".to_string(),
                TestFile::new("x"),
            )]),
            // Symlinks and hardlinks are left out.
            case(vec![(
                "usr/lib/foo/bar.png".to_string(),
                TestFile::symlink("/usr/share/foo/bar.png"),
            )]),
            case(vec![
                ("usr/share/foo/bar.png".to_string(), TestFile::new("x")),
                (
                    "usr/lib/foo/bar.png".to_string(),
                    TestFile::new("x").hardlink_to("usr/share/foo/bar.png"),
                ),
            ]),
        ];
        let fail = vec![
            case(vec![(
                "usr/lib/files-foo-in-bar/bar.png".to_string(),
                TestFile::new("x"),
            )]),
            // Each suffix of the list.
            case(vec![
                ("usr/lib/a/a.bmp".to_string(), TestFile::new("x")),
                ("usr/lib/a/a.gif".to_string(), TestFile::new("x")),
                ("usr/lib/a/a.jpg".to_string(), TestFile::new("x")),
                ("usr/lib/a/a.jpeg".to_string(), TestFile::new("x")),
                ("usr/lib/a/a.tiff".to_string(), TestFile::new("x")),
                ("usr/lib/a/a.xbm".to_string(), TestFile::new("x")),
                ("usr/lib/a/a.xpm".to_string(), TestFile::new("x")),
                ("usr/lib/a/a.xpb".to_string(), TestFile::new("x")),
            ]),
            // A directory with an image name is reported too.
            case(vec![(
                "usr/lib/icons.png".to_string(),
                TestFile::directory(),
            )]),
            // Udebs are checked too.
            case(vec![("usr/lib/a/a.png".to_string(), TestFile::new("x"))]).udeb(),
        ];
        BinaryPackageTester::new(ImageFileInUsrLib, pass, fail).test_and_snapshot();
    }
}
