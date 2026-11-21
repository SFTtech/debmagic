use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuIconNotInXpmFormat;

declare_rule! {
    /// A menu icon does not name an `.xpm` file.
    ///
    /// Checked for `icon`, `icon32x32`, and `icon16x16`. `none` and an empty
    /// value are not this Tag. Leading `/` is stripped before the check, which
    /// ignores case. A name that is not `.xpm` gets no file lookup, so no
    /// missing or parse hint follows it. Extra is the tag and the stripped
    /// path. Location is the menu file and lintian's line. Udebs are checked
    /// too.
    MenuIconNotInXpmFormat,
    code = "LN0967",
    tag = "menu-icon-not-in-xpm-format",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuIconNotInXpmFormat {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_icon_not_in_xpm_format) {
            scan::error(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn item(extra: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/menu/example",
            TestFile::new(format!(
                "?package(example):needs=\"text\" section=\"Applications/System\" title=\"T\" command=\"sh\" {extra}\n"
            )),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            item("icon=\"\""),
            item("icon=\"none\""),
            item("icon=\"/usr/share/pixmaps/a.XPM\""),
            item("icon=\"a.xpm\""),
        ];
        let fail = vec![
            item("icon=\"/usr/share/pixmaps/pargs.png\""),
            item("icon16x16=\"pargs.png\""),
            item("icon32x32=\"/a.PNG\"").udeb(),
        ];
        BinaryPackageTester::new(MenuIconNotInXpmFormat, pass, fail).test_and_snapshot();
    }
}
