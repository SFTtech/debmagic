use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopEntryLacksMainCategory;

declare_rule! {
    /// A desktop entry has `Categories` but no main category.
    ///
    /// Reserved categories (`Screensaver`, `TrayIcon`, `Applet`, `Shell`)
    /// count as main; `X-` categories are skipped. An entry without
    /// `Categories` is not checked. No extra. Location is the desktop file.
    /// Udebs are checked too.
    DesktopEntryLacksMainCategory,
    code = "LN0629",
    tag = "desktop-entry-lacks-main-category",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DesktopEntryLacksMainCategory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_entry_lacks_main_category) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn desktop(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/applications/example.desktop",
            TestFile::new(body),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            desktop("[Desktop Entry]\nType=Application\nName=x\n"),
            desktop("[Desktop Entry]\nType=Application\nCategories=Utility;GTK;\n"),
            desktop(
                "[Desktop Entry]\nType=Application\nCategories=Screensaver;\nOnlyShowIn=GNOME;\n",
            ),
            desktop("[Desktop Entry]\nType=Application\nCategories=GTK;Game\n"),
            desktop("[Desktop Entry]\nType=Bogus\nCategories=GTK;\n"),
        ];
        let fail = vec![
            desktop("[Desktop Entry]\nType=Application\nCategories=GTK;Qt;\n"),
            desktop("[Desktop Entry]\nType=Application\nCategories=\n"),
            desktop("[Desktop Entry]\nType=Application\nCategories=X-Foo;Lintian;\n"),
            desktop("[Desktop Entry]\nType=Application\nCategories=GTK;\n").udeb(),
        ];
        BinaryPackageTester::new(DesktopEntryLacksMainCategory, pass, fail).test_and_snapshot();
    }
}
