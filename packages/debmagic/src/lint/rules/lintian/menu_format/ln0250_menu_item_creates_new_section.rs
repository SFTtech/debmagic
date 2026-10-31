use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuItemCreatesNewSection;

declare_rule! {
    /// A menu item names a section that is not in the menu policy list.
    ///
    /// An unknown root is a different Tag and is not reported here. `Apps/Games`
    /// is rewritten to `Games` and `Apps/` to `Applications/` before the lookup;
    /// `WindowManagers` becomes `Window Managers`. The extra is the original
    /// section value. Location is the menu file and lintian's line.
    MenuItemCreatesNewSection,
    code = "LN0250",
    tag = "menu-item-creates-new-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuItemCreatesNewSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_item_creates_new_section) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::scan::fixture;
    use super::*;

    fn menu(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/share/menu/example", TestFile::new(body))])
    }

    fn item(section: &str) -> BinaryPackageCase {
        menu(&format!(
            "?package(example):needs=\"text\" section=\"{section}\" title=\"T\" command=\"sh\"\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            item("Applications/Editors"),
            item("Help"),
            item("Root"),
            item("Apps/Games/Toys"),
            item("Screen/Saving"),
            item("Window Managers"),
            item("Applications//Editors"),
            item("Applications/Editors/"),
            menu("?package(example):section=\"Games\" title=\"T\" command=\"sh\"\n"),
            menu(
                "!C menu-2\n?package(example):needs=\"text\" section=\"Games\" title=\"T\" command=\"sh\"\n",
            ),
        ];
        let fail = vec![
            item("Games"),
            item("Applications/System"),
            item("Apps/System"),
            item("WindowManagers/Programming"),
            item("Screen"),
            item("Apps/Games"),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/menu-section".to_string(),
                TestFile::new(fixture::MENU_SECTION),
            )]),
            item("Games").udeb(),
        ];
        BinaryPackageTester::new(MenuItemCreatesNewSection, pass, fail).test_and_snapshot();
    }
}
