use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct CommandInMenuFileAndDesktopFile;

declare_rule! {
    /// The same command is listed in a menu file and a desktop file.
    ///
    /// Both names drop a leading `bin/`, `sbin/`, `usr/bin/`, or `usr/sbin/`.
    /// The menu name also drops `usr/games/`. `su-to-root`, `su`, `sux`,
    /// `gksu`, and `kdesu` are not recorded from desktop files. The extra is
    /// the stripped menu command. Location is the menu file and lintian's line.
    CommandInMenuFileAndDesktopFile,
    code = "LN0249",
    tag = "command-in-menu-file-and-desktop-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CommandInMenuFileAndDesktopFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.command_in_menu_file_and_desktop_file) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::scan::fixture;
    use super::*;

    fn with(entries: &[(&str, &str)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, body)| (path.to_string(), TestFile::new(*body)))
                .collect::<Vec<_>>(),
        )
    }

    const MENU: &str = "?package(example):needs=\"text\" section=\"Applications/Editors\" title=\"T\" command=\"/usr/bin/foo\"\n";
    const DESKTOP: &str = "[Desktop Entry]\nType=Application\nName=Foo\nExec=foo\n";

    #[test]
    fn test() {
        let pass = vec![
            with(&[("usr/share/menu/example", MENU)]),
            with(&[
                ("usr/share/menu/example", MENU),
                (
                    "usr/share/applications/foo.desktop",
                    "[Desktop Entry]\nType=Application\nName=Foo\nExec=other\n",
                ),
            ]),
            with(&[
                (
                    "usr/share/menu/example",
                    "?package(example):needs=\"text\" section=\"Applications/Editors\" title=\"T\" command=\"su-to-root\"\n",
                ),
                (
                    "usr/share/applications/foo.desktop",
                    "[Desktop Entry]\nType=Application\nName=Foo\nExec=kdesu\n",
                ),
            ]),
            with(&[
                ("usr/share/menu/example", MENU),
                ("usr/share/xsessions/foo.desktop", DESKTOP),
            ]),
            with(&[
                (
                    "usr/share/menu/example",
                    "?package(example):needs=\"text\" section=\"Applications/Editors\" title=\"T\" command=\"/usr/games/hello\"\n",
                ),
                (
                    "usr/share/applications/foo.desktop",
                    "[Desktop Entry]\nType=Application\nName=Foo\nExec=/usr/games/hello\n",
                ),
            ]),
            with(&[
                ("usr/share/menu/example", MENU),
                ("usr/share/applications/foo-template.desktop", DESKTOP),
            ]),
            with(&[
                ("usr/share/menu/example", MENU),
                (
                    "usr/share/applications/foo.desktop",
                    "[Desktop Entry]\nFoo=Bar\nExec=foo\n",
                ),
            ]),
        ];
        let pargs_desktop = DESKTOP.replace("Exec=foo", "Exec=pargs-section");
        let fail = vec![
            with(&[
                ("usr/share/menu/example", MENU),
                ("usr/share/applications/foo.desktop", DESKTOP),
            ]),
            with(&[
                ("usr/share/menu/menu-section", fixture::MENU_SECTION),
                (
                    "usr/share/applications/pargs.desktop",
                    pargs_desktop.as_str(),
                ),
            ]),
            with(&[
                ("usr/lib/menu/example", MENU),
                ("usr/share/applications/foo.desktop", DESKTOP),
            ]),
        ];
        BinaryPackageTester::new(CommandInMenuFileAndDesktopFile, pass, fail).test_and_snapshot();
    }
}
