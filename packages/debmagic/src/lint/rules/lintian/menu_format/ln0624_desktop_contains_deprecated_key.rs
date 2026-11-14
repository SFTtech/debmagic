use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopContainsDeprecatedKey;

declare_rule! {
    /// A desktop entry starts with the deprecated `[KDE Desktop Entry]` group.
    ///
    /// Only the first group header counts, and only when it is the first
    /// non-comment line. Reported whatever the `Type`. No extra. Location is
    /// the desktop file and the header line. Udebs are checked too.
    DesktopContainsDeprecatedKey,
    code = "LN0624",
    tag = "desktop-contains-deprecated-key",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DesktopContainsDeprecatedKey {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_contains_deprecated_key) {
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
            desktop("# c\n[Desktop Entry]\nType=Application\n[KDE Desktop Entry]\n"),
            desktop("Name=x\n[KDE Desktop Entry]\nType=Application\n"),
        ];
        let fail = vec![
            desktop("[KDE Desktop Entry]\nType=Application\nName=x\n"),
            desktop("# c\n\n[KDE Desktop Entry]  \n"),
            BinaryPackageCase::from(vec![(
                "usr/share/xsessions/example.desktop",
                TestFile::new("[KDE Desktop Entry]\nType=Application\n"),
            )]),
            desktop("[KDE Desktop Entry]\nType=Application\n").udeb(),
        ];
        BinaryPackageTester::new(DesktopContainsDeprecatedKey, pass, fail).test_and_snapshot();
    }
}
