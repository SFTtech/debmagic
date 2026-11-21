use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuIconTooBig;

declare_rule! {
    /// A menu icon's XPM is larger than its tag allows.
    ///
    /// Checked for an icon found in the package, once the file passes the
    /// missing check (LN0505). The limit is 32 for `icon` and `icon32x32`, and
    /// 16 for `icon16x16`. The icon is too big when the width or height on its
    /// XPM size line is above the limit. An unreadable header or size line is
    /// LN0966, not this Tag. Extra is the tag, the stripped path, and
    /// `WIDTHxHEIGHT > LIMITxLIMIT`. Location is the menu file and lintian's
    /// line. An icon provided only by a dependency is not read. Udebs are
    /// checked too.
    MenuIconTooBig,
    code = "LN0968",
    tag = "menu-icon-too-big",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuIconTooBig {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_icon_too_big) {
            scan::error(ctx, &finding);
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

    fn with_file(icon: &str, path: &str, width: u32, height: u32) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![
            ("usr/share/menu/example", TestFile::new(item(icon))),
            (
                path,
                TestFile::new(format!(
                    "/* XPM */\nstatic char *x[] = {{\n\"{width} {height} 2 1\",\n}};\n"
                )),
            ),
        ])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_file(
                "icon=\"/usr/share/pixmaps/a.xpm\"",
                "usr/share/pixmaps/a.xpm",
                32,
                32,
            ),
            with_file("icon32x32=\"a.xpm\"", "usr/share/pixmaps/a.xpm", 16, 8),
            with_file("icon16x16=\"a.xpm\"", "usr/share/pixmaps/a.xpm", 16, 16),
        ];
        let fail = vec![
            with_file(
                "icon32x32=\"/usr/share/pixmaps/a.xpm\"",
                "usr/share/pixmaps/a.xpm",
                256,
                256,
            ),
            with_file("icon16x16=\"a.xpm\"", "usr/share/pixmaps/a.xpm", 32, 16),
            with_file("icon=\"/x/a.xpm\"", "x/a.xpm", 33, 1).udeb(),
        ];
        BinaryPackageTester::new(MenuIconTooBig, pass, fail).test_and_snapshot();
    }
}
