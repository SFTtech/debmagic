use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuItemMissingRequiredTag;

declare_rule! {
    /// A menu item lacks `needs`, `section`, `title`, or `command`.
    ///
    /// The first missing name in that order is the extra, and later checks for
    /// the item are skipped. An empty value still counts as present. An
    /// unparsable item is not this Tag. Location is the menu file and
    /// lintian's line. Udebs are checked too.
    MenuItemMissingRequiredTag,
    code = "LN0517",
    tag = "menu-item-missing-required-tag",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuItemMissingRequiredTag {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_item_missing_required_tag) {
            scan::error(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn menu(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/share/menu/example", TestFile::new(body))])
    }

    fn item(tags: &str) -> BinaryPackageCase {
        menu(&format!("?package(example):{tags}\n"))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            item("needs=\"text\" section=\"Applications/System\" title=\"T\" command=\"sh\""),
            item("needs=\"\" section=\"Applications/System\" title=\"T\" command=\"sh\""),
            item("needs=\" \" section=\"Applications/System\" title=\"T\" command=\"sh\""),
            menu("# comment\n"),
            menu("!C menu-2\n"),
            menu("?package(example):not a menu item\n"),
        ];
        let fail = vec![
            item("needs=\"text\" title=\"T\" command=\"sh\""),
            item("section=\"Applications/System\" title=\"T\" command=\"sh\""),
            item("needs=\"text\" section=\"Applications/System\" command=\"sh\""),
            item("needs=\"text\" section=\"Applications/System\" title=\"T\""),
            item("title=\"T\" command=\"sh\""),
            menu("?package(example):\n"),
            item("needs=text title=\"T\" command=\"sh\""),
            BinaryPackageCase::from(vec![(
                "usr/lib/menu/example",
                TestFile::new(
                    "?package(example):needs=\"text\" section=\"Applications/System\" title=\"T\"\n",
                ),
            )]),
            menu("?package(example):needs=\"text\" title=\"T\" \\\n command=\"sh\"\n"),
            item("needs=\"text\" title=\"Hello World\" command=\"/usr/bin/hello\"").udeb(),
        ];
        BinaryPackageTester::new(MenuItemMissingRequiredTag, pass, fail).test_and_snapshot();
    }
}
