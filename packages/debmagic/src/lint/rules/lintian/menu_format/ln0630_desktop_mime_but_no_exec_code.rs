use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopMimeButNoExecCode;

declare_rule! {
    /// A desktop entry under `usr/share/applications` lists MIME types but
    /// its `Exec` has no `%f`, `%F`, `%u`, or `%U` code.
    ///
    /// A `%%` escape does not count as a code. A missing `Exec` is also
    /// reported. Entries elsewhere and entries with a missing or unknown
    /// `Type` are not checked. No extra. Location is the desktop file. Udebs
    /// are checked too.
    DesktopMimeButNoExecCode,
    code = "LN0630",
    tag = "desktop-mime-but-no-exec-code",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DesktopMimeButNoExecCode {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_mime_but_no_exec_code) {
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
            desktop("[Desktop Entry]\nType=Application\nExec=foo\n"),
            desktop("[Desktop Entry]\nType=Application\nMimeType=text/plain;\nExec=foo %f\n"),
            desktop("[Desktop Entry]\nType=Application\nMimeType=text/plain;\nExec=foo %U\n"),
            desktop("[Desktop Entry]\nType=Application\nMimeType=text/plain;\nExec=%u\n"),
            desktop("[Desktop Entry]\nType=Bogus\nMimeType=text/plain;\nExec=foo\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/xsessions/example.desktop",
                TestFile::new("[Desktop Entry]\nType=Application\nMimeType=a/b;\nExec=foo\n"),
            )]),
        ];
        let fail = vec![
            desktop("[Desktop Entry]\nType=Application\nMimeType=text/plain;\nExec=foo\n"),
            desktop("[Desktop Entry]\nType=Application\nMimeType=text/plain;\n"),
            desktop("[Desktop Entry]\nType=Application\nMimeType=text/plain;\nExec=foo %%f %i\n"),
            desktop("[Desktop Entry]\nType=Application\nMimeType=text/plain;\nExec=foo\n").udeb(),
        ];
        BinaryPackageTester::new(DesktopMimeButNoExecCode, pass, fail).test_and_snapshot();
    }
}
