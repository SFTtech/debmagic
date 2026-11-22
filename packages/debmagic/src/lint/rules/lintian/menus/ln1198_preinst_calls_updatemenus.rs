use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct PreinstCallsUpdatemenus;

declare_rule! {
    /// `preinst` calls `update-menus`.
    ///
    /// Menus are registered after installation, so `preinst` must not call
    /// `update-menus`, whether or not the package ships a menu. `command`,
    /// `which` and `-x` probes do not count, and `/usr/bin/update-menus` does.
    /// Only the last call is reported. Extra is empty. Location is `preinst`
    /// and that line.
    PreinstCallsUpdatemenus,
    code = "LN1198",
    tag = "preinst-calls-updatemenus",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PreinstCallsUpdatemenus {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        if let Some(line) = scan::update_menus_line(files, "preinst") {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: PathBuf::from("preinst"),
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

    fn preinst(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("preinst", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            preinst("#!/bin/sh\ntrue\n"),
            preinst("#!/bin/sh\n# update-menus\n"),
            preinst("#!/bin/sh\ncommand update-menus\n"),
            preinst("#!/bin/sh\nupdate-menus-foo\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::new("#!/bin/sh\nupdate-menus\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("preinst", TestFile::symlink("missing")),
        ];
        let fail = vec![
            preinst("#!/bin/sh\nupdate-menus\n"),
            preinst(
                "#!/bin/sh\nset -e\n\nif [ -x /usr/bin/update-menus ]; then\n    update-menus\nfi\n",
            ),
            preinst("#!/bin/sh\nupdate-menus\nset -e\n/usr/bin/update-menus\n"),
            preinst("update-menus\n").udeb(),
            preinst("#!/bin/sh\nif which update-menus; then update-menus; fi\n"),
        ];
        BinaryPackageTester::new(PreinstCallsUpdatemenus, pass, fail).test_and_snapshot();
    }
}
