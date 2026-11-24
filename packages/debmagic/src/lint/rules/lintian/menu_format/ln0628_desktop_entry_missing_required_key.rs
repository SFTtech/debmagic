use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopEntryMissingRequiredKey;

declare_rule! {
    /// A desktop entry lacks `Type` or `Name`.
    ///
    /// Only entries with a known `Type` are checked, so in practice this is
    /// `Name`. A localized `Name[de]` does not count. Extra is the key.
    /// Location is the desktop file. Udebs are checked too.
    DesktopEntryMissingRequiredKey,
    code = "LN0628",
    tag = "desktop-entry-missing-required-key",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DesktopEntryMissingRequiredKey {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_entry_missing_required_key) {
            scan::error(ctx, &finding);
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
            desktop("[Desktop Entry]\nType=Link\nName=\n"),
            desktop("[Desktop Entry]\nName=x\n"),
            desktop("[Desktop Entry]\nType=Bogus\n"),
            desktop("[Desktop Entry]\nType=Application\nName = x\n"),
        ];
        let fail = vec![
            desktop("[Desktop Entry]\nType=Application\n"),
            desktop("[Desktop Entry]\nType=Directory\nName[de]=x\n"),
            desktop("[Desktop Entry]\nType=Application\n[Other]\nName=x\n"),
            desktop("[KDE Desktop Entry]\nType=Link\n"),
            desktop("[Desktop Entry]\nType=Application\n").udeb(),
        ];
        BinaryPackageTester::new(DesktopEntryMissingRequiredKey, pass, fail).test_and_snapshot();
    }
}
