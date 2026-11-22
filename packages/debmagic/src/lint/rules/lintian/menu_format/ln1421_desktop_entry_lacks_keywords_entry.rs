use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopEntryLacksKeywordsEntry;

declare_rule! {
    /// An `Application` desktop entry without `NoDisplay` has no `Keywords`
    /// key.
    ///
    /// Reported as an info hint with no Extra. Location is the desktop file,
    /// with no line. Only `Type=Application` is checked; an entry whose `Type`
    /// is missing or unknown is skipped, as are entries with `NoDisplay` set
    /// (any value). Only the exact key `Keywords` counts, not a localized one.
    DesktopEntryLacksKeywordsEntry,
    code = "LN1421",
    tag = "desktop-entry-lacks-keywords-entry",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for DesktopEntryLacksKeywordsEntry {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_entry_lacks_keywords_entry) {
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
            desktop("[Desktop Entry]\nType=Application\nName=x\nKeywords=a;b;\n"),
            desktop("[Desktop Entry]\nType=Application\nName=x\nNoDisplay=true\n"),
            // Only Application entries are checked.
            desktop("[Desktop Entry]\nType=Application\nName=x\nNoDisplay=false\n"),
            desktop("[Desktop Entry]\nType=Link\nName=x\n"),
            desktop("[Desktop Entry]\nType=Bogus\nName=x\n"),
        ];
        let fail = vec![
            desktop("[Desktop Entry]\nType=Application\nName=x\n"),
            desktop("[Desktop Entry]\nType=Application\nName=x\nKeywords[de]=a;\n"),
            desktop("[Desktop Entry]\nType=Application\nName=x\n").udeb(),
        ];
        BinaryPackageTester::new(DesktopEntryLacksKeywordsEntry, pass, fail).test_and_snapshot();
    }
}
