use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct PostrmDoesNotCallUpdatemenus;

declare_rule! {
    /// A menu method is installed and `postrm` never runs `update-menus`.
    ///
    /// A menu file alone does not trigger this. The package `menu` is exempt.
    /// The call is recognized the same way as in `postinst`. Extra is empty.
    /// Location is the last sorted menu method, with no line.
    PostrmDoesNotCallUpdatemenus,
    code = "LN0327",
    tag = "postrm-does-not-call-updatemenus",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PostrmDoesNotCallUpdatemenus {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name() == "menu" {
            return;
        }
        let anchor = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            scan::menu_method_paths(files).last().cloned()
        };
        let Some(path) = anchor else {
            return;
        };
        let calls = ctx
            .control_files()
            .is_some_and(|files| scan::calls_update_menus(files, "postrm"));
        if calls {
            return;
        }
        ctx.diagnostic(Diagnostic::error("").with_location(Location {
            path,
            line: None,
            column: None,
        }));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn method(name: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            format!("etc/menu-methods/{name}"),
            TestFile::new("!include notmenu.h\n"),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/example",
                TestFile::new("?package(example):needs=text\n"),
            )]),
            method("lintian").control_file("postrm", TestFile::new("#!/bin/sh\nupdate-menus\n")),
            method("lintian")
                .control_file(
                    "postrm",
                    TestFile::new("#!/bin/sh\nexec /usr/bin/update-menus\n"),
                )
                .package("menu"),
            method("lintian").package("menu"),
            BinaryPackageCase::from(vec![(
                "etc/menu-methods/lintian",
                TestFile::symlink("missing"),
            )]),
        ];
        let fail = vec![
            method("lintian"),
            method("lintian").control_file("postrm", TestFile::new("#!/bin/sh\ntrue\n")),
            method("lintian")
                .control_file("postinst", TestFile::new("#!/bin/sh\nupdate-menus\n"))
                .control_file("postrm", TestFile::symlink("missing")),
            BinaryPackageCase::from(vec![
                ("etc/menu-methods/a", TestFile::new("x\n")),
                ("etc/menu-methods/lintian", TestFile::new("y\n")),
            ]),
        ];
        BinaryPackageTester::new(PostrmDoesNotCallUpdatemenus, pass, fail).test_and_snapshot();
    }
}
