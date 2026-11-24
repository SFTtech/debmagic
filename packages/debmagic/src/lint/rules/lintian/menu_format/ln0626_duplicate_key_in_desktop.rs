use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DuplicateKeyInDesktop;

declare_rule! {
    /// A desktop entry sets the same key twice.
    ///
    /// Keys are compared with their locale, so `Name` and `Name[de]` are
    /// different. Only the first group is read. Reported whatever the `Type`.
    /// Extra is the key. Location is the desktop file and the line of the
    /// repeat. Udebs are checked too.
    DuplicateKeyInDesktop,
    code = "LN0626",
    tag = "duplicate-key-in-desktop",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DuplicateKeyInDesktop {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.duplicate_key_in_desktop) {
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
            desktop("[Desktop Entry]\nType=Application\nName=x\nName[de]=y\n"),
            desktop("[Desktop Entry]\nType=Application\n[Other]\nName=x\nName=y\n"),
            desktop("[Desktop Entry]\nType=Application\n#Name=x\nName=y\n"),
        ];
        let fail = vec![
            desktop("[Desktop Entry]\nName=general\nName=general\nType=Application\n"),
            desktop(
                "[Desktop Entry]\nType=Application\nName[de]=a\nName[de]=b\nExec=x\nExec = y\n",
            ),
            desktop("[Desktop Entry]\nName=a\nName=b\nName=c\n"),
            desktop("[Desktop Entry]\nType=Bogus\nExec=a\nExec=b\n").udeb(),
        ];
        BinaryPackageTester::new(DuplicateKeyInDesktop, pass, fail).test_and_snapshot();
    }
}
