use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuItemUsesAppsGamesSection;

declare_rule! {
    /// A menu item's section starts with the old `Apps/Games` name.
    ///
    /// Checked on the section after duplicate slashes are squeezed. Only a
    /// section that starts with `Apps/Games` is this Tag. A plain `Apps/` is
    /// LN0253, and `Applications/Games` is current and is not reported. No
    /// extra. Location is the menu file and lintian's line. Udebs are checked
    /// too.
    MenuItemUsesAppsGamesSection,
    code = "LN0972",
    tag = "menu-item-uses-apps-games-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuItemUsesAppsGamesSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_item_uses_apps_games_section) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn menu(section: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/menu/example",
            TestFile::new(format!(
                "?package(example):needs=\"text\" section=\"{section}\" title=\"T\" command=\"sh\"\n"
            )),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            menu("Applications/Games"),
            menu("Apps/System"),
        ];
        let fail = vec![menu("Apps/Games/Board"), menu("Apps//Games").udeb()];
        BinaryPackageTester::new(MenuItemUsesAppsGamesSection, pass, fail).test_and_snapshot();
    }
}
