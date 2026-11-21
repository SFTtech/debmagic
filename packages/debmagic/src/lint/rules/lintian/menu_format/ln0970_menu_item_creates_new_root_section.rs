use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuItemCreatesNewRootSection;

declare_rule! {
    /// A menu item's section root is not a known menu root.
    ///
    /// The section is taken after lintian's historical renames (`Apps/`
    /// becomes `Applications/`, `WindowManagers` becomes `Window Managers`, and
    /// `Apps/Games` loses its `Apps/`), with duplicate slashes squeezed. The
    /// root is the text before the first `/`. A root that is in lintian's
    /// menu-section list is not this Tag, and neither is a root containing the
    /// package name, ignoring case. Extra is the root, empty when the section
    /// has no root. Location is the menu file and lintian's line. Udebs are
    /// checked too. The package name match is a plain substring, where lintian
    /// uses it as a regex.
    MenuItemCreatesNewRootSection,
    code = "LN0970",
    tag = "menu-item-creates-new-root-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuItemCreatesNewRootSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_item_creates_new_root_section) {
            scan::error(ctx, &finding);
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
            menu("Applications/System"),
            menu("Apps/System"),
            // The root contains the package name, ignoring case.
            menu("Example/Tools"),
        ];
        let fail = vec![menu("Root/Thing"), menu("Other").udeb()];
        BinaryPackageTester::new(MenuItemCreatesNewRootSection, pass, fail).test_and_snapshot();
    }
}
