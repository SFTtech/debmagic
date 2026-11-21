use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuIconCannotBeParsed;

declare_rule! {
    /// A menu icon `.xpm` in the package has no XPM header or no size line.
    ///
    /// Checked for `icon`, `icon32x32`, and `icon16x16` once the file is found
    /// in the package (the file is looked up as the stripped path, then under
    /// `usr/share/pixmaps/`). A missing icon is LN0505, and `none`, an empty
    /// value, and a non-`.xpm` name are not read. The file is scanned for a
    /// `/* XPM */` header line, then for the first later line with
    /// `"width height colors chars"`. Extra is the tag, the stripped path, and
    /// `looking for XPM header` or `looking for size line`. Location is the
    /// menu file and lintian's line. An icon provided only by a dependency is
    /// not read. Udebs are checked too.
    MenuIconCannotBeParsed,
    code = "LN0966",
    tag = "menu-icon-cannot-be-parsed",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuIconCannotBeParsed {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_icon_cannot_be_parsed) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn item(extra: &str) -> String {
        format!(
            "?package(example):needs=\"text\" section=\"Applications/System\" title=\"T\" command=\"sh\" {extra}\n"
        )
    }

    fn with_file(body: impl Into<String>, path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![
            ("usr/share/menu/example", TestFile::new(body.into())),
            (path, file),
        ])
    }

    fn xpm() -> TestFile {
        TestFile::new("/* XPM */\nstatic char *x[] = {\n\"16 16 2 1\",\n};\n")
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_file(
                item("icon=\"/usr/share/pixmaps/ok.xpm\""),
                "usr/share/pixmaps/ok.xpm",
                xpm(),
            ),
            with_file(item("icon=\"ok.xpm\""), "usr/share/pixmaps/ok.xpm", xpm()),
            // A missing icon is menu-icon-missing, not this Tag.
            BinaryPackageCase::from(vec![(
                "usr/share/menu/example",
                TestFile::new(item("icon=\"/usr/share/pixmaps/gone.xpm\"")),
            )]),
        ];
        let fail = vec![
            with_file(
                item("icon=\"/usr/share/pixmaps/broken.xpm\""),
                "usr/share/pixmaps/broken.xpm",
                TestFile::new("P2\n16 16\n"),
            ),
            with_file(
                item("icon16x16=\"/usr/share/pixmaps/broken.xpm\""),
                "usr/share/pixmaps/broken.xpm",
                TestFile::new("/* XPM */\nno size line here\n"),
            ),
            with_file(
                item("icon=\"/usr/share/pixmaps/broken.xpm\""),
                "usr/share/pixmaps/broken.xpm",
                TestFile::new("/* XPM */\n\"x\"\n"),
            )
            .udeb(),
        ];
        BinaryPackageTester::new(MenuIconCannotBeParsed, pass, fail).test_and_snapshot();
    }
}
