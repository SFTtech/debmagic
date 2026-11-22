use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopEntryLimitedToEnvironments;

declare_rule! {
    /// A desktop entry whose `OnlyShowIn` names more than one environment,
    /// outside a reserved category, should use `NotShowIn` instead.
    ///
    /// Reported as an info hint with no Extra. Location is the desktop file,
    /// with no line. `OnlyShowIn` is split on `;` with trailing empty fields
    /// dropped, as Perl's `split` does. An entry that lists a reserved
    /// category in `Categories` is skipped, as lintian sets `in_reserved`
    /// there. Only files the menu-format Check reads are checked.
    DesktopEntryLimitedToEnvironments,
    code = "LN1576",
    tag = "desktop-entry-limited-to-environments",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for DesktopEntryLimitedToEnvironments {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_entry_limited_to_environments) {
            scan::info(ctx, &finding);
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
            desktop("[Desktop Entry]\nType=Application\nName=x\nOnlyShowIn=GNOME;\n"),
            // One environment is fine, whatever NoDisplay says.
            desktop(
                "[Desktop Entry]\nType=Application\nName=x\nOnlyShowIn=GNOME;\nNoDisplay=true\n",
            ),
            // A reserved category exempts the entry.
            desktop(
                "[Desktop Entry]\nType=Application\nName=x\nOnlyShowIn=GNOME;KDE;\nCategories=Shell;\n",
            ),
            desktop("[Desktop Entry]\nType=Application\nName=x\nNotShowIn=GNOME;KDE;\n"),
        ];
        let fail = vec![
            desktop("[Desktop Entry]\nType=Application\nName=x\nOnlyShowIn=GNOME;KDE;\n"),
            desktop("[Desktop Entry]\nType=Application\nName=x\nOnlyShowIn=GNOME;KDE\n"),
            desktop("[Desktop Entry]\nType=Application\nName=x\nOnlyShowIn=GNOME;KDE;\n").udeb(),
        ];
        BinaryPackageTester::new(DesktopEntryLimitedToEnvironments, pass, fail).test_and_snapshot();
    }
}
