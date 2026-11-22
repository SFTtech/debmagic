use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopEntryLacksIconEntry;

declare_rule! {
    /// A desktop entry without `NoDisplay` has no `Icon` key.
    ///
    /// Reported as an info hint with no Extra. Location is the desktop file,
    /// with no line. Only `.desktop` files that the menu-format Check reads
    /// (directly under `usr/share/applications` and `usr/share/xsessions`)
    /// are checked, and a file with an unknown or missing `Type` is skipped,
    /// as lintian returns before this test. Any value of `NoDisplay` counts as
    /// set. Only the exact key `Icon` counts, not a localized `Icon[de]`.
    DesktopEntryLacksIconEntry,
    code = "LN1575",
    tag = "desktop-entry-lacks-icon-entry",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for DesktopEntryLacksIconEntry {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_entry_lacks_icon_entry) {
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
            desktop("[Desktop Entry]\nType=Application\nName=x\nIcon=foo\n"),
            desktop("[Desktop Entry]\nType=Application\nName=x\nNoDisplay=true\n"),
            // NoDisplay set to any value, even false, suppresses the hint.
            desktop("[Desktop Entry]\nType=Application\nName=x\nNoDisplay=false\n"),
            // An unknown Type returns before the test.
            desktop("[Desktop Entry]\nType=Bogus\nName=x\n"),
        ];
        let fail = vec![
            desktop("[Desktop Entry]\nType=Application\nName=x\n"),
            desktop("[Desktop Entry]\nType=Link\nName=x\nIcon[de]=foo\n"),
            desktop("[Desktop Entry]\nType=Application\nName=x\n").udeb(),
        ];
        BinaryPackageTester::new(DesktopEntryLacksIconEntry, pass, fail).test_and_snapshot();
    }
}
