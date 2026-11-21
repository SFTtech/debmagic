use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuItemNeedsDwww;

declare_rule! {
    /// A menu item outside the window manager sections uses `needs="dwww"`.
    ///
    /// `needs` is compared case-insensitively. Items in the Window Managers
    /// and Window Managers/Modules sections are checked by their own Tags
    /// (LN0255, LN0620), so they are not this Tag. For other sections lintian
    /// reports `dwww` here instead of as an unknown `needs` value (LN0443).
    /// No extra. Location is the menu file and lintian's line. Udebs are
    /// checked too.
    MenuItemNeedsDwww,
    code = "LN0971",
    tag = "menu-item-needs-dwww",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuItemNeedsDwww {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_item_needs_dwww) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn menu(needs: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/menu/example",
            TestFile::new(format!(
                "?package(example):needs=\"{needs}\" section=\"Applications/System\" title=\"T\" command=\"sh\"\n"
            )),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            menu("text"),
            menu("x11"),
        ];
        let fail = vec![menu("dwww"), menu("DWWW").udeb()];
        BinaryPackageTester::new(MenuItemNeedsDwww, pass, fail).test_and_snapshot();
    }
}
