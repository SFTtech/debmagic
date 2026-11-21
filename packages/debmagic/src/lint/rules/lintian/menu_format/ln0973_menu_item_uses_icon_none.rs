use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuItemUsesIconNone;

declare_rule! {
    /// A menu icon tag is set to the word `none`.
    ///
    /// Checked for `icon`, `icon32x32`, and `icon16x16`. Each tag whose value
    /// is exactly `none` is reported with its tag name. An empty value is not
    /// this Tag. The icon checks for that tag stop at `none`, so no relative
    /// path, not-in-xpm, missing, or parse hint follows it. Extra is the tag
    /// name. Location is the menu file and lintian's line. Udebs are checked
    /// too.
    MenuItemUsesIconNone,
    code = "LN0973",
    tag = "menu-item-uses-icon-none",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuItemUsesIconNone {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_item_uses_icon_none) {
            scan::warn(ctx, &finding);
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
            item("icon=\"None\""),
            item("icon=\"/usr/share/pixmaps/a.xpm\""),
        ];
        let fail = vec![
            item("icon=\"none\""),
            item("icon=\"none\" icon16x16=\"none\""),
            item("icon32x32=\"none\"").udeb(),
        ];
        BinaryPackageTester::new(MenuItemUsesIconNone, pass, fail).test_and_snapshot();
    }
}
