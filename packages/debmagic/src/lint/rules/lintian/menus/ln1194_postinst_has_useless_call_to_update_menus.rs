use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct PostinstHasUselessCallToUpdateMenus;

declare_rule! {
    /// `postinst` calls `update-menus` although the package has no menu.
    ///
    /// A menu file (other than `README`) under `usr/lib/menu` or
    /// `usr/share/menu`, or a menu method under `etc/menu-methods`, keeps the
    /// call needed. Without one, `update-menus` (also `/usr/bin/update-menus`)
    /// in `postinst` is useless. A `command`, `which` or `-x` probe does not
    /// count. Only the last call is reported. Extra is empty. Location is
    /// `postinst` and that line.
    PostinstHasUselessCallToUpdateMenus,
    code = "LN1194",
    tag = "postinst-has-useless-call-to-update-menus",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PostinstHasUselessCallToUpdateMenus {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let has_menu = ctx
            .data_files()
            .is_some_and(|files| scan::menu_anchor(files).is_some());
        if has_menu {
            return;
        }
        let Some(files) = ctx.control_files() else {
            return;
        };
        if let Some(line) = scan::update_menus_line(files, "postinst") {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("postinst"),
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn postinst(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("postinst", TestFile::new(body))
    }

    fn with_menu(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/share/menu/example", TestFile::new("x"))])
            .control_file("postinst", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            postinst("#!/bin/sh\ntrue\n"),
            postinst("#!/bin/sh\n# update-menus\n"),
            postinst("#!/bin/sh\ncommand update-menus\n"),
            // A menu file or a menu method makes the call useful.
            with_menu("#!/bin/sh\nupdate-menus\n"),
            BinaryPackageCase::from(vec![("etc/menu-methods/example", TestFile::new("x"))])
                .control_file("postinst", TestFile::new("#!/bin/sh\nupdate-menus\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::symlink("missing")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postrm", TestFile::new("#!/bin/sh\nupdate-menus\n")),
        ];
        let fail = vec![
            postinst("#!/bin/sh\nupdate-menus\n"),
            postinst("#!/bin/sh\nupdate-menus\nset -e\n/usr/bin/update-menus\n"),
            postinst(
                "#!/bin/sh\nset -e\n\nif [ -x /usr/bin/update-menus ]; then\n    update-menus\nfi\n",
            ),
            postinst("update-menus\n").udeb(),
            // The README is not a menu file.
            BinaryPackageCase::from(vec![("usr/share/menu/README", TestFile::new("x"))])
                .control_file("postinst", TestFile::new("#!/bin/sh\nupdate-menus\n")),
            postinst("#!/bin/sh\nif which update-menus; then update-menus; fi\n"),
        ];
        BinaryPackageTester::new(PostinstHasUselessCallToUpdateMenus, pass, fail)
            .test_and_snapshot();
    }
}
