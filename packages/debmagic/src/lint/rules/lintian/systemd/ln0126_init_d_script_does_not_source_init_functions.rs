use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::systemd_init;

pub struct InitDScriptDoesNotSourceInitFunctions;

declare_rule! {
    /// An `etc/init.d` script does not source the lsb init functions.
    ///
    /// A first line of `init-d-script`, or `.` / `source` of
    /// `/lib/lsb/init-functions` or `init-d-script` (optional `/usr`), counts.
    /// The same scripts are skipped as for the missing-service Tag, including
    /// `initscripts`. Extra is empty; the script path is the Location.
    InitDScriptDoesNotSourceInitFunctions,
    code = "LN0126",
    tag = "init.d-script-does-not-source-init-functions",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptDoesNotSourceInitFunctions {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let (_any_service, hits) = systemd_init::init_scripts(files, &package);
        for hit in hits {
            if hit.sources_init {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: hit.path,
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

    fn script(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(format!("etc/init.d/{name}"), TestFile::new(body))])
    }

    #[test]
    fn test() {
        let pass = vec![
            script("ok", "#!/bin/sh\n. /lib/lsb/init-functions\n"),
            script(
                "new",
                "#!/usr/lib/init/init-d-script\n### BEGIN INIT INFO\n",
            ),
            script("skeleton", "#!/bin/sh\nexit 0\n"),
        ];
        let fail = vec![
            script("no-lsb", "#!/bin/sh\nexit 0\n"),
            script(
                "commented",
                "#!/bin/sh\n# . /lib/lsb/init-functions\nexit 0\n",
            ),
        ];
        BinaryPackageTester::new(InitDScriptDoesNotSourceInitFunctions, pass, fail)
            .test_and_snapshot();
    }
}
