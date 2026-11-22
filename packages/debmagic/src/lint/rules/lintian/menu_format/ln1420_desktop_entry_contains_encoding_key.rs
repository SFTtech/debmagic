use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopEntryContainsEncodingKey;

declare_rule! {
    /// A desktop entry sets the `Encoding` key, which the FreeDesktop
    /// specification deprecated: every string is UTF-8 now.
    ///
    /// Reported as an info hint. Nothing is reported for an entry whose `Type`
    /// is missing or unknown, as for the deprecated keys. Extra is the key as
    /// written, including any locale. Location is the desktop file and the line.
    /// Udebs are checked too.
    DesktopEntryContainsEncodingKey,
    code = "LN1420",
    tag = "desktop-entry-contains-encoding-key",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for DesktopEntryContainsEncodingKey {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_entry_contains_encoding_key) {
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
            desktop("[Desktop Entry]\nType=Application\nName=x\nExec=x\n"),
            // Without a known Type, nothing is reported.
            desktop("[Desktop Entry]\nType=Bogus\nEncoding=UTF-8\n"),
            // Another section does not count.
            desktop("[Desktop Entry]\nType=Application\n[Other]\nEncoding=UTF-8\n"),
        ];
        let fail = vec![
            desktop("[Desktop Entry]\nType=Application\nEncoding=UTF-8\n"),
            desktop("[Desktop Entry]\nType=Application\nName=x\nEncoding[de]=UTF-8\n"),
            desktop("[Desktop Entry]\nType=Link\nEncoding=UTF-8\n").udeb(),
        ];
        BinaryPackageTester::new(DesktopEntryContainsEncodingKey, pass, fail).test_and_snapshot();
    }
}
