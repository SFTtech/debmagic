use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct NonWmModuleInWmModulesMenuSection;

declare_rule! {
    /// A window manager module menu item uses a regular `needs` value or `wm`.
    ///
    /// Sections starting with `WindowManagers/Modules`, `FVWM Modules`, or
    /// `Window Maker`. `x11`, `text`, `vc`, and `wm` (any case) are reported.
    /// Extra is `needs` in lower case. Location is the menu file and
    /// lintian's line. Udebs are checked too.
    NonWmModuleInWmModulesMenuSection,
    code = "LN0620",
    tag = "non-wm-module-in-wm-modules-menu-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NonWmModuleInWmModulesMenuSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| {
            report.non_wm_module_in_wm_modules_menu_section
        }) {
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

    fn item(section: &str, needs: &str) -> BinaryPackageCase {
        menu(&format!(
            "?package(example):needs=\"{needs}\" section=\"{section}\" title=\"T\" command=\"sh\"\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            item("WindowManagers/Modules", "dwww"),
            item("FVWM Modules", "fvwm"),
            item("Window Maker", "windowmaker"),
            item("Window Managers", "wm"),
            item("Applications/Editors", "wm"),
        ];
        let fail = vec![
            item("WindowManagers/Modules", "wm"),
            item("FVWM Modules", "X11"),
            item("Window Maker", "text"),
            item("Window Maker/Sub", "vc"),
            BinaryPackageCase::from(vec![(
                "usr/lib/menu/example".to_string(),
                TestFile::new(
                    "# comment\n?package(example):needs=\"wm\" section=\"FVWM Modules\" title=\"T\" command=\"sh\"\n",
                ),
            )]),
            item("Window Maker", "wm").udeb(),
        ];
        BinaryPackageTester::new(NonWmModuleInWmModulesMenuSection, pass, fail).test_and_snapshot();
    }
}
