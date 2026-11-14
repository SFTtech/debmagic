use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuCommandNotInPackage;

declare_rule! {
    /// A menu item's command is not shipped by the package.
    ///
    /// The extra is the command after wrapper unwrapping and one leading `/`
    /// removed. Suppressed when the `?package` test names two or more packages,
    /// or the section is `WindowManagers/Modules`, `FVWM Modules`, or `Window
    /// Maker`. Location is the menu file and lintian's line.
    MenuCommandNotInPackage,
    code = "LN0248",
    tag = "menu-command-not-in-package",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuCommandNotInPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.menu_command_not_in_package) {
            scan::warn(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::scan::fixture;
    use super::*;

    fn menu(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/share/menu/example", TestFile::new(body))])
    }

    fn with(entries: &[(&str, &str)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, body)| (path.to_string(), TestFile::new(*body)))
                .collect::<Vec<_>>(),
        )
    }

    const ITEM: &str =
        "?package(example):needs=\"text\" section=\"Applications/Editors\" title=\"T\" command=";

    #[test]
    fn test() {
        let pass = vec![
            with(&[
                (
                    "usr/share/menu/example",
                    &format!("{ITEM}\"/usr/bin/hello\"\n"),
                ),
                ("usr/bin/hello", "#!/bin/sh\n"),
            ]),
            with(&[
                ("usr/share/menu/example", &format!("{ITEM}\"hello\"\n")),
                ("usr/games/hello", "#!/bin/sh\n"),
            ]),
            menu(&format!("{ITEM}\"sh\"\n")),
            menu(&format!("{ITEM}\"/bin/sh\"\n")),
            menu(&format!("{ITEM}\"sensible-editor\"\n")),
            menu(&format!("{ITEM}\"'missing'\"\n")),
            menu(
                "?package(example, other):needs=\"text\" section=\"Applications/Editors\" title=\"T\" command=\"missing\"\n",
            ),
            menu(
                "?package(example):needs=\"text\" section=\"Window Maker\" title=\"T\" command=\"missing\"\n",
            ),
            menu(
                "?package(example):needs=\"text\" section=\"FVWM Modules\" title=\"T\" command=\"missing\"\n",
            ),
            menu(
                "?package(example):needs=\"text\" section=\"WindowManagers/Modules\" title=\"T\" command=\"missing\"\n",
            ),
            menu(
                "!C menu-2\n?package(example):needs=\"text\" section=\"Applications/Editors\" title=\"T\" command=\"missing\"\n",
            ),
            menu("?package(example):title=\"T\" command=\"missing\"\n"),
            with(&[(
                "usr/share/menu/README",
                "?package(example):needs=\"text\" section=\"Applications/Editors\" title=\"T\" command=\"missing\"\n",
            )]),
            with(&[(
                "usr/share/menu/extra/item",
                "?package(example):needs=\"text\" section=\"Applications/Editors\" title=\"T\" command=\"missing\"\n",
            )]),
            with(&[
                ("usr/share/menu/menu-section", fixture::MENU_SECTION),
                ("usr/bin/pargs-section", "#!/bin/sh\n"),
            ])
            .package("menu-section"),
        ];
        let fail = vec![
            menu(&format!("{ITEM}\"/usr/bin/not-here\"\n")),
            menu(
                "?package(example):needs=\"text\" section=\"Applications/Editors\" title=\"T\" command=\"/usr/sbin/su-to-root -c xfdisk\"\n",
            ),
            with(&[(
                "usr/lib/menu/example",
                "?package(example):needs=\"text\" section=\"Applications/Editors\" title=\"T\" command=\"/imnothere\"\n",
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/example".to_string(),
                TestFile::new(format!("{ITEM}\"/usr/bin/not-here\"\n")).hardlink(),
            )]),
            menu(&format!("{ITEM}\"/usr/bin/not-here\"\n")).udeb(),
            menu(
                "# comment\n\n?package(example):needs=\"text\" section=\"Applications/Editors\" title=\"T\" command=\"/usr/bin/not-here\"\n",
            ),
            with(&[("usr/share/menu/menu-section", fixture::MENU_SECTION)]).package("menu-section"),
        ];
        BinaryPackageTester::new(MenuCommandNotInPackage, pass, fail).test_and_snapshot();
    }
}
