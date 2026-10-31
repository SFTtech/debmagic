use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct PostinstDoesNotCallUpdatemenus;

declare_rule! {
    /// A menu file or menu method is installed and `postinst` never runs `update-menus`.
    ///
    /// The anchor is the last sorted menu file under `usr/lib/menu` or
    /// `usr/share/menu`, excluding `README`, or else the last menu method.
    /// `update-menus` and `/usr/bin/update-menus` count after a comment is
    /// stripped, including after `then`, `do`, or `exec`. A symlink `postinst`
    /// does not count. Extra is empty. Location is the anchor, with no line.
    PostinstDoesNotCallUpdatemenus,
    code = "LN0326",
    tag = "postinst-does-not-call-updatemenus",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PostinstDoesNotCallUpdatemenus {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let anchor = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            scan::menu_anchor(files)
        };
        let Some(path) = anchor else {
            return;
        };
        let calls = ctx
            .control_files()
            .is_some_and(|files| scan::calls_update_menus(files, "postinst"));
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

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| {
                    (
                        (*path).to_string(),
                        TestFile::new("?package(example):needs=text\n"),
                    )
                })
                .collect::<Vec<_>>(),
        )
    }

    fn postinst(case: BinaryPackageCase, body: &str) -> BinaryPackageCase {
        case.control_file("postinst", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            files(&["usr/lib/menu/README", "usr/share/menu/README"]),
            BinaryPackageCase::from(vec![(
                "usr/share/menu/example",
                TestFile::symlink("missing"),
            )]),
            postinst(
                files(&["usr/share/menu/example"]),
                "#!/bin/sh\nupdate-menus\n",
            ),
            postinst(
                files(&["usr/share/menu/example"]),
                "#!/bin/sh\n/usr/bin/update-menus\n",
            ),
            postinst(
                files(&["usr/share/menu/example"]),
                "#!/bin/sh\nexec update-menus\n",
            ),
            postinst(
                files(&["usr/share/menu/example"]),
                "#!/bin/sh\nupdate-menus # refresh\n",
            ),
            postinst(
                files(&["etc/menu-methods/lintian"]),
                "#!/bin/sh\nif which update-menus >/dev/null; then\n    update-menus\nfi\n",
            )
            .package("menu"),
        ];
        let fail = vec![
            files(&["usr/share/menu/example"]),
            files(&["usr/lib/menu/example"]),
            files(&["usr/lib/menu/example", "usr/share/menu/example"]),
            files(&["usr/share/menu/README", "usr/lib/menu/example"]),
            files(&["usr/share/menu/README.Debian"]),
            files(&["etc/menu-methods/lintian"]),
            postinst(
                files(&["usr/share/menu/example"]),
                "#!/bin/sh\n# update-menus\n",
            ),
            postinst(
                files(&["usr/share/menu/example"]),
                "#!/bin/sh\nnoupdate-menus\n",
            ),
            files(&["usr/share/menu/example"])
                .control_file("postinst", TestFile::symlink("missing")),
            files(&["usr/share/menu/example"]).udeb(),
            files(&["usr/share/menu/example"])
                .control_file("postinst", TestFile::new("#!/bin/sh\ntrue\n"))
                .control_file("prerm", TestFile::new("#!/bin/sh\nupdate-menus\n")),
            files(&["etc/menu-methods/lintian"]).package("menu"),
        ];
        BinaryPackageTester::new(PostinstDoesNotCallUpdatemenus, pass, fail).test_and_snapshot();
    }
}
