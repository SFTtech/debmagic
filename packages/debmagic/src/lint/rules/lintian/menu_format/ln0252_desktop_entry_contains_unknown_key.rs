use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopEntryContainsUnknownKey;

declare_rule! {
    /// A desktop entry uses a key outside the FreeDesktop, KDE, and `X-` sets.
    ///
    /// Deprecated keys are not unknown. The locale suffix is stripped before
    /// the lookup; the extra is the key as written. Reported only for a known
    /// `Type`, and not for keys in a later group. Location is the desktop file
    /// and the key's line.
    DesktopEntryContainsUnknownKey,
    code = "LN0252",
    tag = "desktop-entry-contains-unknown-key",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DesktopEntryContainsUnknownKey {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_entry_contains_unknown_key) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::scan::fixture;
    use super::*;

    fn desktop(path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(body))])
    }

    const APP: &str = "[Desktop Entry]\nType=Application\nName=Foo\n";

    #[test]
    fn test() {
        let pass = vec![
            desktop("usr/share/applications/foo.desktop", APP),
            desktop(
                "usr/share/applications/foo.desktop",
                &format!("{APP}X-Foo=Bar\nDev=1\nEncoding=UTF-8\nName[fr]=Bonjour\n"),
            ),
            desktop(
                "usr/share/applications/foo.desktop",
                "[Desktop Entry]\nName=Foo\nFoo=Bar\n",
            ),
            desktop(
                "usr/share/applications/foo.desktop",
                "[Desktop Entry]\nType=Bogus\nName=Foo\nFoo=Bar\n",
            ),
            desktop(
                "usr/share/applications/foo.desktop",
                "[Desktop Entry]\nType=Application\nName=Foo\n[Other]\nFoo=Bar\n",
            ),
            desktop(
                "usr/share/applications/mytemplate.desktop",
                &format!("{APP}Foo=Bar\n"),
            ),
            desktop("usr/share/applications/foo.desktop", "hello\nFoo=Bar\n"),
            desktop(
                "usr/share/xsessions/foo.desktop",
                "[KDE Desktop Entry]\nType=Application\nName=Foo\n",
            ),
        ];
        let fail = vec![
            desktop(
                "usr/share/applications/foo.desktop",
                fixture::GENERAL_DESKTOP,
            ),
            desktop(
                "usr/share/applications/foo.desktop",
                &format!("{APP}icon=hello\n"),
            ),
            desktop(
                "usr/share/applications/foo.desktop",
                &format!("{APP}Foo[en]=Bar\n"),
            ),
            desktop(
                "usr/share/applications/foo.desktop",
                &format!("{APP}Foo=1\nFoo=2\n"),
            ),
            desktop(
                "usr/share/xsessions/foo.desktop",
                &format!("{APP}SpecialTag=1\n"),
            ),
            desktop(
                "usr/share/applications/foo.desktop",
                "[Desktop Entry]\r\nType=Application\r\nName=Foo\r\nFoo=Bar\r\n",
            ),
        ];
        BinaryPackageTester::new(DesktopEntryContainsUnknownKey, pass, fail).test_and_snapshot();
    }
}
