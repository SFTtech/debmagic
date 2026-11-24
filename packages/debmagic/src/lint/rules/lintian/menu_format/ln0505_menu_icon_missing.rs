use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuIconMissing;

declare_rule! {
    /// A menu icon in `.xpm` form is not a file in this package.
    ///
    /// Checked for `icon`, `icon32x32`, and `icon16x16` after the required tags
    /// are present. `none` and an empty value are not this Tag, and a path that
    /// does not end in `.xpm` is not this Tag. Leading `/` are stripped. The
    /// file counts when that path, or `usr/share/pixmaps/` plus that path, is a
    /// regular file or hardlink, following symlinks. A directory does not
    /// count. Another installable in the same upload is not consulted. Extra
    /// is the tag and the stripped path. Location is the menu file and
    /// lintian's line. Udebs are checked too.
    MenuIconMissing,
    code = "LN0505",
    tag = "menu-icon-missing",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuIconMissing {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_icon_missing) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn item(icons: &str) -> String {
        format!(
            "?package(example):needs=\"text\" section=\"Applications/System\" title=\"T\" command=\"sh\" {icons}\n"
        )
    }

    fn menu(body: impl Into<String>) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/share/menu/example", TestFile::new(body.into()))])
    }

    fn with_file(body: impl Into<String>, path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![
            ("usr/share/menu/example", TestFile::new(body.into())),
            (path, file),
        ])
    }

    #[test]
    fn test() {
        let xpm = TestFile::new("/* XPM */\n");
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            menu(item("icon=\"none\"")),
            menu(item("icon=\"\"")),
            menu(item("icon=\"/usr/share/pixmaps/missing.png\"")),
            menu("!C menu-2\n"),
            menu(item("icon=\"/nope.xpm\"").replace("needs=\"text\" ", "")),
            with_file(
                item("icon=\"/usr/share/pixmaps/ok.xpm\""),
                "usr/share/pixmaps/ok.xpm",
                xpm.clone(),
            ),
            with_file(
                item("icon=\"ok.xpm\""),
                "usr/share/pixmaps/ok.xpm",
                xpm.clone(),
            ),
            with_file(item("icon=\"ok.xpm\""), "ok.xpm", xpm.clone()),
            BinaryPackageCase::from(vec![
                (
                    "usr/share/menu/example",
                    TestFile::new(item("icon=\"/usr/share/pixmaps/link.xpm\"")),
                ),
                ("usr/share/pixmaps/ok.xpm", xpm.clone()),
                ("usr/share/pixmaps/link.xpm", TestFile::symlink("ok.xpm")),
            ]),
            with_file(
                item("icon=\"/usr/share/pixmaps/hard.xpm\""),
                "usr/share/pixmaps/hard.xpm",
                xpm.hardlink(),
            ),
            with_file(
                item("icon=\"/usr/share/pixmaps/OK.XPM\""),
                "usr/share/pixmaps/OK.XPM",
                TestFile::new("/* XPM */\n"),
            ),
        ];
        let fail = vec![
            menu(item(
                "icon=\"/usr/share/menu-none/non-existent.xpm\" icon16x16=\"/usr/share/menu-none/non-existent16.xpm\" icon32x32=\"/usr/share/menu-none/non-existent32.xpm\"",
            )),
            menu(item(
                "icon=\"non-existent.xpm\" icon16x16=\"non-existent16.xpm\" icon32x32=\"non-existent32.xpm\"",
            )),
            menu(item("icon=\"//usr/share/pixmaps/nope.xpm\"")),
            menu(item("icon=\"Missing.XPM\"")),
            menu(item("icon=\"none.xpm\"")),
            with_file(
                item("icon=\"/usr/share/pixmaps/dir.xpm\""),
                "usr/share/pixmaps/dir.xpm",
                TestFile::directory(),
            ),
            with_file(
                item("icon=\"/usr/share/pixmaps/broken.xpm\""),
                "usr/share/pixmaps/broken.xpm",
                TestFile::symlink("absent.xpm"),
            ),
            BinaryPackageCase::from(vec![
                (
                    "usr/share/menu/example",
                    TestFile::new(item("icon=\"/usr/share/pixmaps/dir-link.xpm\"")),
                ),
                ("usr/share/pixmaps/real-dir", TestFile::directory()),
                (
                    "usr/share/pixmaps/dir-link.xpm",
                    TestFile::symlink("real-dir"),
                ),
            ]),
            BinaryPackageCase::from(vec![(
                "usr/lib/menu/example",
                TestFile::new(item("icon=\"/usr/lib/menu/missing.xpm\"")),
            )]),
            menu(
                "?package(example):needs=\"text\" section=\"Applications/System\" title=\"T\" \\\n command=\"sh\" icon=\"/nope.xpm\"\n",
            ),
            menu(item("icon=\"/nope.xpm\"")).udeb(),
        ];
        BinaryPackageTester::new(MenuIconMissing, pass, fail).test_and_snapshot();
    }
}
