use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::systemd_init::{self, whitelisted};

pub struct MissingSystemdServiceForInitDScript;

declare_rule! {
    /// An `etc/init.d` script has no systemd unit anywhere in the package.
    ///
    /// `README`, `skeleton`, `rc`, and `rcS` are skipped, as are scripts whose
    /// link target is `lib/init/upstart-job`, the `initscripts` package, and
    /// names in systemd's init whitelist. An rcS `Default-Start` is the other
    /// Tag. When the package already ships any `.service` file, this Tag is
    /// silent. Extra is the service name (a trailing `.sh` dropped). The
    /// script path is the Location. Udebs are checked too.
    MissingSystemdServiceForInitDScript,
    code = "LN0125",
    tag = "missing-systemd-service-for-init.d-script",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MissingSystemdServiceForInitDScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let (any_service, hits) = systemd_init::init_scripts(files, &package);
        if any_service {
            return;
        }
        for hit in hits {
            if hit.rc_s || whitelisted(&hit.service) {
                continue;
            }
            ctx.diagnostic(Diagnostic::error(hit.service).with_location(Location {
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
        BinaryPackageCase::from(vec![(
            format!("etc/init.d/{name}"),
            TestFile::new(format!("#!/bin/sh\n{body}")),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("skeleton", "exit 0\n"),
            script("rc.local", "exit 0\n"),
            script("boot", "# Default-Start: S\nexit 0\n"),
            BinaryPackageCase::from(vec![
                (
                    "etc/init.d/example".to_string(),
                    TestFile::new("#!/bin/sh\nexit 0\n"),
                ),
                (
                    "lib/systemd/system/example.service".to_string(),
                    TestFile::new("[Service]\nExecStart=/bin/true\n"),
                ),
            ]),
            script("example", "exit 0\n").package("initscripts"),
        ];
        let fail = vec![
            script("no-lsb", "exit 0\n"),
            script("lsb-other", ". /lib/lsb/init-functions\n"),
        ];
        BinaryPackageTester::new(MissingSystemdServiceForInitDScript, pass, fail)
            .test_and_snapshot();
    }
}
