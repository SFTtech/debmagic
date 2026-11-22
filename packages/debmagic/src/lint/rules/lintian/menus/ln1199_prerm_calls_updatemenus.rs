use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct PrermCallsUpdatemenus;

declare_rule! {
    /// `prerm` calls `update-menus`.
    ///
    /// Menus are handled after the package is unpacked or removed, so `prerm`
    /// must not call `update-menus`, whether or not the package ships a menu.
    /// `command`, `which` and `-x` probes do not count, and
    /// `/usr/bin/update-menus` does. Only the last call is reported. Extra is
    /// empty. Location is `prerm` and that line.
    PrermCallsUpdatemenus,
    code = "LN1199",
    tag = "prerm-calls-updatemenus",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrermCallsUpdatemenus {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        if let Some(line) = scan::update_menus_line(files, "prerm") {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: PathBuf::from("prerm"),
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

    fn prerm(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("prerm", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            prerm("#!/bin/sh\ntrue\n"),
            prerm("#!/bin/sh\n# update-menus\n"),
            prerm("#!/bin/sh\ncommand update-menus\n"),
            prerm("#!/bin/sh\nupdate-menus-foo\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postrm", TestFile::new("#!/bin/sh\nupdate-menus\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("prerm", TestFile::symlink("missing")),
        ];
        let fail = vec![
            prerm("#!/bin/sh\nupdate-menus\n"),
            prerm(
                "#!/bin/sh\nset -e\n\nif [ -x /usr/bin/update-menus ]; then\n    update-menus\nfi\n",
            ),
            prerm("#!/bin/sh\nupdate-menus\nset -e\n/usr/bin/update-menus\n"),
            prerm("update-menus\n").udeb(),
            prerm("#!/bin/sh\nif which update-menus; then update-menus; fi\n"),
        ];
        BinaryPackageTester::new(PrermCallsUpdatemenus, pass, fail).test_and_snapshot();
    }
}
