use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuItemUsesAppsSection;

declare_rule! {
    /// A menu item still uses the old `Apps/` section.
    ///
    /// `Apps/Games` is the older games rename and is not this Tag. The extra
    /// is empty. Location is the menu file and lintian's line.
    MenuItemUsesAppsSection,
    code = "LN0253",
    tag = "menu-item-uses-apps-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuItemUsesAppsSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_item_uses_apps_section) {
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
            item("Apps/Games"),
            item("Apps/Games/Toys"),
            menu("?package(example):section=\"Apps/Editors\" title=\"T\" command=\"sh\"\n"),
            menu(
                "!C menu-2\n?package(example):needs=\"text\" section=\"Apps/Editors\" title=\"T\" command=\"sh\"\n",
            ),
        ];
        let fail = vec![
            item("Apps/Programming"),
            item("Apps/System"),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/menu-section".to_string(),
                TestFile::new(fixture::MENU_SECTION),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/menu/example".to_string(),
                TestFile::new(
                    "?package(example):needs=\"text\" section=\"Apps/System\" title=\"T\" command=\"sh\"\n",
                ),
            )]),
            item("Apps/Editors").udeb(),
        ];
        BinaryPackageTester::new(MenuItemUsesAppsSection, pass, fail).test_and_snapshot();
    }
}
