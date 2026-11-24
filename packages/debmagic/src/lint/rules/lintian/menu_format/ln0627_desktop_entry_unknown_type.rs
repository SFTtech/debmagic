use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopEntryUnknownType;

declare_rule! {
    /// A desktop entry has a `Type` other than `Application`, `Link`, or
    /// `Directory`.
    ///
    /// A missing `Type` is not this Tag. Extra is the type. Location is the
    /// desktop file. Udebs are checked too.
    DesktopEntryUnknownType,
    code = "LN0627",
    tag = "desktop-entry-unknown-type",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DesktopEntryUnknownType {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_entry_unknown_type) {
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
            desktop("[Desktop Entry]\nType=Link\n"),
            desktop("[Desktop Entry]\nType=Directory\n"),
            desktop("[Desktop Entry]\nName=x\n"),
            desktop("[Other]\nType=foo\n"),
        ];
        let fail = vec![
            desktop("[Desktop Entry]\nType=foo\n"),
            desktop("[Desktop Entry]\nType=application\nName=x\n"),
            desktop("[Desktop Entry]\nType=\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/xsessions/example.desktop",
                TestFile::new("[Desktop Entry]\nType=Session\n"),
            )]),
            desktop("[Desktop Entry]\nType=foo\n").udeb(),
        ];
        BinaryPackageTester::new(DesktopEntryUnknownType, pass, fail).test_and_snapshot();
    }
}
