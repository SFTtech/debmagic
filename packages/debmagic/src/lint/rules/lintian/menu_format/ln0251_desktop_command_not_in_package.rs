use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct DesktopCommandNotInPackage;

declare_rule! {
    /// A desktop entry's `Exec` command is not shipped by the package.
    ///
    /// Only files directly under `usr/share/applications`, and only when `Type`
    /// is known. `kcmshell` is allowed. The extra is the command after wrapper
    /// unwrapping and one leading `/` removed. Location is the desktop file,
    /// with no line.
    DesktopCommandNotInPackage,
    code = "LN0251",
    tag = "desktop-command-not-in-package",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DesktopCommandNotInPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.desktop_command_not_in_package) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::scan::fixture;
    use super::*;

    fn desktop(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/applications/foo.desktop",
            TestFile::new(body),
        )])
    }

    fn with(entries: &[(&str, &str)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, body)| (path.to_string(), TestFile::new(*body)))
                .collect::<Vec<_>>(),
        )
    }

    const APP: &str = "[Desktop Entry]\nType=Application\nName=Foo\n";

    #[test]
    fn test() {
        let pass = vec![
            with(&[
                (
                    "usr/share/applications/foo.desktop",
                    &format!("{APP}Exec=foo\n"),
                ),
                ("usr/bin/foo", "#!/bin/sh\n"),
            ]),
            desktop(&format!("{APP}Exec=kcmshell\n")),
            desktop(&format!("{APP}Exec=sh\n")),
            desktop(&format!("{APP}Exec=/usr/bin/sensible-browser\n")),
            desktop(&format!("{APP}Exec='missing'\n")),
            with(&[
                (
                    "usr/share/applications/foo.desktop",
                    &format!("{APP}Exec=/usr/bin/kdesu -c hello\n"),
                ),
                ("usr/bin/hello", "#!/bin/sh\n"),
            ]),
            with(&[(
                "usr/share/xsessions/foo.desktop",
                &format!("{APP}Exec=missing\n"),
            )]),
            desktop("[Desktop Entry]\nType=Application\nName=Foo\n"),
            desktop("[Desktop Entry]\nName=Foo\nExec=missing\n"),
            desktop("[Desktop Entry]\nType=Bogus\nName=Foo\nExec=missing\n"),
            with(&[(
                "usr/share/applications/kde/foo.desktop",
                &format!("{APP}Exec=missing\n"),
            )]),
            with(&[(
                "usr/share/applications/foo-template.desktop",
                &format!("{APP}Exec=missing\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/applications/foo.desktop".to_string(),
                TestFile::symlink("missing"),
            )]),
        ];
        let fail = vec![
            desktop(&format!("{APP}Exec=foo\n")),
            desktop(&format!("{APP}Exec=/usr/bin/missing\n")),
            desktop(fixture::GENERAL_DESKTOP),
            BinaryPackageCase::from(vec![(
                "usr/share/applications/foo.desktop".to_string(),
                TestFile::new(format!("{APP}Exec=foo\n")).hardlink(),
            )]),
            desktop(&format!("{APP}Exec=foo\n")).udeb(),
            desktop(&format!("{APP}Exec=sux -p missing\n")),
        ];
        BinaryPackageTester::new(DesktopCommandNotInPackage, pass, fail).test_and_snapshot();
    }
}
