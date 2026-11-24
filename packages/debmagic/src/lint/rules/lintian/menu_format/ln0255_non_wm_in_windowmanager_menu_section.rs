use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct NonWmInWindowmanagerMenuSection;

declare_rule! {
    /// A Window Managers menu item does not set `needs=wm`.
    ///
    /// `WindowManagers/Modules`, `FVWM Modules`, and `Window Maker` are a
    /// different Tag. The extra is `needs` in lower case. Location is the menu
    /// file and lintian's line.
    NonWmInWindowmanagerMenuSection,
    code = "LN0255",
    tag = "non-wm-in-windowmanager-menu-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NonWmInWindowmanagerMenuSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.non_wm_in_windowmanager_menu_section) {
            scan::error(ctx, &finding);
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

    fn item(section: &str, needs: &str) -> BinaryPackageCase {
        menu(&format!(
            "?package(example):needs=\"{needs}\" section=\"{section}\" title=\"T\" command=\"sh\"\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            item("Window Managers", "wm"),
            item("WindowManagers", "WM"),
            item("WindowManagers/Modules", "x11"),
            item("Window Maker", "x11"),
            item("FVWM Modules", "text"),
            item("Applications/Editors", "x11"),
            menu("?package(example):needs=\"x11\" title=\"T\" command=\"sh\"\n"),
        ];
        let fail = vec![
            item("Window Managers", "x11"),
            item("WindowManagers", "X11"),
            item("WindowManagers/Programming", "text"),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/menu-section".to_string(),
                TestFile::new(fixture::MENU_SECTION),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/menu/example".to_string(),
                TestFile::new(
                    "?package(example):needs=\"x11\" section=\"Window Managers\" title=\"T\" command=\"sh\"\n",
                ),
            )]),
            item("Window Managers", "vc").udeb(),
        ];
        BinaryPackageTester::new(NonWmInWindowmanagerMenuSection, pass, fail).test_and_snapshot();
    }
}
