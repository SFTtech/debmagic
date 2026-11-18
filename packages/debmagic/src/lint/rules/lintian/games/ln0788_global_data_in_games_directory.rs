use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct GlobalDataInGamesDirectory;

declare_rule! {
    /// A non-directory under `usr/share/games/applications`, `mime`, `icons`,
    /// or `pixmaps`.
    ///
    /// Those directories hold global system data that belongs directly under
    /// `usr/share`. The directories themselves are not flagged, and neither is
    /// a file named like one of them directly under `usr/share/games`. Symbolic
    /// links are flagged too. Extra is empty; the path is the Location.
    /// Udebs are checked.
    GlobalDataInGamesDirectory,
    code = "LN0788",
    tag = "global-data-in-games-directory",
    default_selected = true,
    experimental = false,
}

static GLOBAL_DATA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/share/games/(?:applications|mime|icons|pixmaps)/").expect("valid regex")
});

impl BinaryPackageRule for GlobalDataInGamesDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|path| GLOBAL_DATA.is_match(path))
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
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

    fn tree(paths: &[(&str, TestFile)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|(path, file)| ((*path).to_string(), file.clone()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            tree(&[
                ("usr/share/games/fortunes/x", TestFile::new("x")),
                ("usr/share/games/applications", TestFile::directory()),
                ("usr/share/games/icons/hicolor", TestFile::directory()),
                ("usr/share/applications/x.desktop", TestFile::new("x")),
                ("usr/games/applications/x", TestFile::new("x")),
            ]),
            // A file named like a directory, directly under `usr/share/games`.
            tree(&[("usr/share/games/applications", TestFile::new("x"))]),
        ];
        let fail = vec![
            tree(&[(
                "usr/share/games/applications/blaster.desktop",
                TestFile::new("x"),
            )]),
            tree(&[("usr/share/games/mime/blaster.xml", TestFile::new("x"))]),
            tree(&[(
                "usr/share/games/icons/hicolor/22x22/lintian-22x22.png",
                TestFile::new("x"),
            )]),
            tree(&[("usr/share/games/pixmaps/blaster.xpm", TestFile::new("x"))]),
            tree(&[(
                "usr/share/games/icons/link.png",
                TestFile::symlink("hicolor/blaster.png"),
            )]),
            tree(&[("usr/share/games/mime/blaster.xml", TestFile::new("x"))]).udeb(),
        ];
        BinaryPackageTester::new(GlobalDataInGamesDirectory, pass, fail).test_and_snapshot();
    }
}
