use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopEntryContainsDeprecatedKey;

declare_rule! {
    /// A desktop entry sets a key the specification deprecated.
    ///
    /// `Encoding` is a different Tag. Nothing is reported for an entry whose
    /// `Type` is missing or unknown. A repeated key is a different Tag. Extra
    /// is the key including any locale. Location is the desktop file and the
    /// line. Udebs are checked too.
    DesktopEntryContainsDeprecatedKey,
    code = "LN0625",
    tag = "desktop-entry-contains-deprecated-key",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DesktopEntryContainsDeprecatedKey {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_entry_contains_deprecated_key) {
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
            desktop("[Desktop Entry]\nType=Application\nName=x\nExec=x\n"),
            desktop("[Desktop Entry]\nType=Application\nEncoding=UTF-8\n"),
            desktop("[Desktop Entry]\nTerminalOptions=-g\n"),
            desktop("[Desktop Entry]\nType=Bogus\nTerminalOptions=-g\n"),
            desktop("[Desktop Entry]\nType=Application\n[Other]\nMiniIcon=x\n"),
        ];
        let fail = vec![
            desktop("[Desktop Entry]\nType=Application\nTerminalOptions=-g 80x25\n"),
            desktop(
                "[Desktop Entry]\nType=Application\nName=x\nMiniIcon=a\nMiniIcon[de]=b\nProtocols=ftp\n",
            ),
            desktop("[KDE Desktop Entry]\nType=Application\nMiniIcon=a\n"),
            desktop("[Desktop Entry]\nType=Link\nTerminalOptions=-g\n").udeb(),
        ];
        BinaryPackageTester::new(DesktopEntryContainsDeprecatedKey, pass, fail).test_and_snapshot();
    }
}
