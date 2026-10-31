use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuItemUsesWindowmanagersSection;

declare_rule! {
    /// A menu item still uses the old `WindowManagers` section.
    ///
    /// `Window Managers` (with the space) is the current name and is not this
    /// Tag. The extra is empty. Location is the menu file and lintian's line.
    MenuItemUsesWindowmanagersSection,
    code = "LN0254",
    tag = "menu-item-uses-windowmanagers-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuItemUsesWindowmanagersSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_item_uses_windowmanagers_section) {
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
            "?package(example):needs=\"wm\" section=\"{section}\" title=\"T\" command=\"sh\"\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            item("Window Managers"),
            item("Applications/Editors"),
            item("FVWM Modules"),
            item("Window Maker"),
            menu("?package(example):title=\"T\" command=\"sh\" section=\"WindowManagers\"\n"),
        ];
        let fail = vec![
            item("WindowManagers"),
            item("WindowManagers/Modules"),
            item("WindowManagers/Programming"),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/menu-section".to_string(),
                TestFile::new(fixture::MENU_SECTION),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/menu/example".to_string(),
                TestFile::new(
                    "?package(example):needs=\"wm\" section=\"WindowManagers\" title=\"T\" command=\"sh\"\n",
                ),
            )]),
        ];
        BinaryPackageTester::new(MenuItemUsesWindowmanagersSection, pass, fail).test_and_snapshot();
    }
}
