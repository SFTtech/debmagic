use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MenuMethodLacksInclude;

declare_rule! {
    /// A menu method does not include `menu.h`.
    ///
    /// A line must start with `!include menu.h`. The package `menu` is exempt.
    /// Extra is empty. Location is the menu method, with no line.
    MenuMethodLacksInclude,
    code = "LN0333",
    tag = "menu-method-lacks-include",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MenuMethodLacksInclude {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name() == "menu" {
            return;
        }
        let lacking = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            scan::menu_method_paths(files)
                .into_iter()
                .filter(|path| scan::lacks_menu_h(files, path))
                .collect::<Vec<_>>()
        };
        for path in lacking {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn method(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            format!("etc/menu-methods/{name}"),
            TestFile::new(body),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            method("lintian", "!include menu.h\ncompat=\"menu-2\"\n"),
            method("lintian", "!include menu.h extra\n"),
            method("lintian", "x\n!include menu.h\n"),
            method("lintian", "!include notmenu.h\n").package("menu"),
            BinaryPackageCase::from(vec![(
                "etc/menu-methods/lintian",
                TestFile::symlink("missing"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/example",
                TestFile::new("!include notmenu.h\n"),
            )]),
        ];
        let fail = vec![
            method("lintian", "!include notmenu.h\n"),
            method("lintian", ""),
            method("lintian", " !include menu.h\n"),
            BinaryPackageCase::from(vec![
                ("etc/menu-methods/a", TestFile::new("nope\n")),
                ("etc/menu-methods/b", TestFile::new("!include menu.h\n")),
                ("etc/menu-methods/c", TestFile::new("also\n")),
            ]),
        ];
        BinaryPackageTester::new(MenuMethodLacksInclude, pass, fail).test_and_snapshot();
    }
}
