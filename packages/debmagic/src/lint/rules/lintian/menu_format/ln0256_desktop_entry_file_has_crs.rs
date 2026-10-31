use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopEntryFileHasCrs;

declare_rule! {
    /// A desktop entry line contains a carriage return.
    ///
    /// Reported once, on the first such line, including comments and a file
    /// that is not a desktop entry. The extra is empty. Location is the
    /// desktop file and that line.
    DesktopEntryFileHasCrs,
    code = "LN0256",
    tag = "desktop-entry-file-has-crs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DesktopEntryFileHasCrs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_entry_file_has_crs) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn desktop(path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(body))])
    }

    #[test]
    fn test() {
        let pass = vec![
            desktop(
                "usr/share/applications/foo.desktop",
                "[Desktop Entry]\nType=Application\nName=Foo\n",
            ),
            desktop("usr/share/applications/foo.desktop", "hello\n"),
            desktop(
                "usr/share/xsessions/foo.desktop",
                "[Desktop Entry]\nType=Application\nName=Foo\n",
            ),
            desktop("usr/share/applications/nested/foo.desktop", "hello\r\n"),
            desktop("usr/share/doc/foo.desktop", "hello\r\n"),
        ];
        let fail = vec![
            desktop(
                "usr/share/applications/crlf.desktop",
                "[Desktop Entry]\r\nName=general\r\nType=Application\r\n",
            ),
            desktop(
                "usr/share/applications/hello.desktop",
                "# comment\n\n# [Foo Bar]\n[KDE Desktop Entry]\nName=Hello\nName[en_US]=Hello\nComment=Say hello!\r\nExec=kdesu hello\nType=Application\n",
            ),
            desktop(
                "usr/share/applications/foo.desktop",
                "# comment\r\n[Desktop Entry]\nType=Application\nName=Foo\n",
            ),
            desktop("usr/share/applications/foo.desktop", "hello\r\nFoo=Bar\r\n"),
            desktop(
                "usr/share/xsessions/foo.desktop",
                "[Desktop Entry]\r\nType=Application\nName=Foo\n",
            ),
            desktop(
                "usr/share/applications/foo.desktop",
                "[Desktop Entry]\r\nType=Application\nName=Foo\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(DesktopEntryFileHasCrs, pass, fail).test_and_snapshot();
    }
}
