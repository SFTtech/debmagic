use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopEntryUsesReservedCategory;

declare_rule! {
    /// A desktop entry uses a reserved category without `OnlyShowIn`.
    ///
    /// Reserved categories are Screensaver, TrayIcon, Applet, and Shell. An
    /// empty or `0` `OnlyShowIn` still warns. The extra is the category.
    /// Location is the desktop file, with no line.
    DesktopEntryUsesReservedCategory,
    code = "LN0258",
    tag = "desktop-entry-uses-reserved-category",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DesktopEntryUsesReservedCategory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_entry_uses_reserved_category) {
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
            "usr/share/applications/foo.desktop",
            TestFile::new(body),
        )])
    }

    fn cats(body: &str) -> BinaryPackageCase {
        desktop(&format!(
            "[Desktop Entry]\nType=Application\nName=Foo\n{body}"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            cats("Categories=Screensaver;\nOnlyShowIn=GNOME;\n"),
            cats("OnlyShowIn=KDE;\nCategories=Applet;\n"),
            cats("Categories=Utility;\n"),
            cats("Categories=X-Screensaver;\n"),
            desktop("[Desktop Entry]\nName=Foo\nCategories=Screensaver;\n"),
            desktop("[Desktop Entry]\nType=Link\nName=Foo\nCategories=Shell;\nOnlyShowIn=GNOME;\n"),
        ];
        let fail = vec![
            cats("Categories=Screensaver;\n"),
            cats("Categories=TrayIcon;\n"),
            cats("Categories=Applet;\n"),
            cats("Categories=Shell;\n"),
            cats("Categories=Screensaver;\nOnlyShowIn=\n"),
            cats("Categories=Screensaver;\nOnlyShowIn=0\n"),
            cats("Categories=Screensaver;Applet;\n"),
            cats("Categories=Screensaver;\n").udeb(),
        ];
        BinaryPackageTester::new(DesktopEntryUsesReservedCategory, pass, fail).test_and_snapshot();
    }
}
