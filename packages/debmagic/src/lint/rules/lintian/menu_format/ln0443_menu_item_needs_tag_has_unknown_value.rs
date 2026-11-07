use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuItemNeedsTagHasUnknownValue;

declare_rule! {
    /// A menu item's `needs` is not `x11`, `text`, or `vc`.
    ///
    /// Window-manager sections are different Tags. `needs=dwww` is
    /// `menu-item-needs-dwww`. Extra is `needs` in lower case. Location is the
    /// menu file and lintian's line. Udebs are checked too.
    MenuItemNeedsTagHasUnknownValue,
    code = "LN0443",
    tag = "menu-item-needs-tag-has-unknown-value",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuItemNeedsTagHasUnknownValue {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_item_needs_tag_has_unknown_value) {
            scan::warn(ctx, &finding);
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

    fn item(section: &str, needs: &str) -> BinaryPackageCase {
        menu(&format!(
            "?package(example):needs=\"{needs}\" section=\"{section}\" title=\"T\" command=\"sh\"\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            item("Applications/Editors", "x11"),
            item("Applications/Editors", "text"),
            item("Applications/Editors", "vc"),
            item("Applications/Editors", "dwww"),
            item("Window Managers", "wm"),
            item("WindowManagers/Modules", "x11"),
        ];
        let fail = vec![
            item("Applications/Editors", "wm"),
            item("Applications/Editors", "gnome"),
            item("Games", "fvwmmodule"),
            item("Applications/Editors", "wm").udeb(),
        ];
        BinaryPackageTester::new(MenuItemNeedsTagHasUnknownValue, pass, fail).test_and_snapshot();
    }
}
