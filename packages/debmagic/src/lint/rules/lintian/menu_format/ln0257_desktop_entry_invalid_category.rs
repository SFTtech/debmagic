use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopEntryInvalidCategory;

declare_rule! {
    /// A desktop category is neither a main nor an additional category.
    ///
    /// `X-` names are skipped. Reserved categories are a different Tag. The
    /// extra is the category. Location is the desktop file, with no line.
    /// Reported only for a known `Type`.
    DesktopEntryInvalidCategory,
    code = "LN0257",
    tag = "desktop-entry-invalid-category",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DesktopEntryInvalidCategory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_entry_invalid_category) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::scan::fixture;
    use super::*;

    fn desktop(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/applications/foo.desktop",
            TestFile::new(body),
        )])
    }

    fn cats(categories: &str) -> BinaryPackageCase {
        desktop(&format!(
            "[Desktop Entry]\nType=Application\nName=Foo\nCategories={categories}\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            cats("Utility;Development;"),
            cats("Application;"),
            cats("GNUstep;"),
            cats("X-Foo;Building;"),
            cats("Screensaver;"),
            cats("AudioVideo;"),
            desktop("[Desktop Entry]\nName=Foo\nCategories=Lintian;\n"),
            desktop("[Desktop Entry]\nType=Bogus\nName=Foo\nCategories=Lintian;\n"),
            desktop("[Desktop Entry]\nType=Application\nName=Foo\n"),
        ];
        let fail = vec![
            desktop(fixture::GENERAL_DESKTOP),
            cats("utility"),
            cats("WeirdStuff;Lintian;"),
            cats("Audio;;Video"),
            cats("Lintian").udeb(),
        ];
        BinaryPackageTester::new(DesktopEntryInvalidCategory, pass, fail).test_and_snapshot();
    }
}
