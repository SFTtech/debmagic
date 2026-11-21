use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuItemContainsUnknownTag;

declare_rule! {
    /// A menu item sets a tag that lintian does not know.
    ///
    /// Checked once the required tags (`needs`, `section`, `title`, and
    /// `command`) are all present, since an item missing one is LN0517 and
    /// stops there. Known tags are `needs`, `section`, `title`, `sort`,
    /// `command`, `longtitle`, `icon`, `icon16x16`, `icon32x32`,
    /// `description`, `hotkey`, and `hints`. Each unknown tag is reported once,
    /// in sorted order. Extra is the tag name. Location is the menu file and
    /// lintian's line. Udebs are checked too.
    MenuItemContainsUnknownTag,
    code = "LN0969",
    tag = "menu-item-contains-unknown-tag",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuItemContainsUnknownTag {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_item_contains_unknown_tag) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn menu(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/menu/example",
            TestFile::new(format!("?package(example):{body}\n")),
        )])
    }

    const REQUIRED: &str =
        "needs=\"text\" section=\"Applications/System\" title=\"T\" command=\"sh\"";

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            menu(&format!(
                "{REQUIRED} longtitle=\"L\" sort=\"a\" description=\"d\" hotkey=\"h\" hints=\"x\" icon=\"\""
            )),
            // A missing required tag stops before the unknown-tag check.
            menu("needs=\"text\" section=\"Applications/System\" custom=\"x\""),
        ];
        let fail = vec![
            menu(&format!("{REQUIRED} custom=\"x\"")),
            menu(&format!("{REQUIRED} zeta=\"1\" alpha=\"2\"")),
            menu(&format!("{REQUIRED} custom=\"x\"")).udeb(),
        ];
        BinaryPackageTester::new(MenuItemContainsUnknownTag, pass, fail).test_and_snapshot();
    }
}
