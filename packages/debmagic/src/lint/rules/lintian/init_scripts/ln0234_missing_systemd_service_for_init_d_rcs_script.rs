use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::{init_script, systemd_unit};

pub struct MissingSystemdServiceForInitDRcSScript;

declare_rule! {
    /// An rcS `etc/init.d` script has no matching systemd service.
    ///
    /// rcS means a line matches `#.*Default-Start:.*S`. `README`, `skeleton`,
    /// `rc`, and `rcS` are skipped, as are the `initscripts` package and names
    /// in systemd's init whitelist. A trailing `.sh` is dropped from the
    /// service name. A unit basename (`@` before `.service` dropped) or an
    /// `Alias=` provides that name; other units in the package do not hide
    /// this Tag. Extra is the service name. The script path is the Location.
    /// Udebs are checked too.
    MissingSystemdServiceForInitDRcSScript,
    code = "LN0234",
    tag = "missing-systemd-service-for-init.d-rcS-script",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MissingSystemdServiceForInitDRcSScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let provided = systemd_unit::provided_service_names(files);
        let scripts = init_script::rc_s_scripts(files, &package);
        for script in scripts {
            if init_script::whitelisted(&script.service) || provided.contains(&script.service) {
                continue;
            }
            ctx.diagnostic(Diagnostic::error(script.service).with_location(at(script.path)));
        }
    }
}

fn at(path: PathBuf) -> Location {
    Location {
        path,
        line: None,
        column: None,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn rcs(name: &str) -> String {
        format!("#!/bin/sh\n# Default-Start:     S 2 3 4 5\nexit 0\n# {name}\n")
    }

    fn script(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(format!("etc/init.d/{name}"), TestFile::new(body))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("plain", "#!/bin/sh\n# Default-Start:     2 3 4 5\n"),
            script("lower", "#!/bin/sh\n# Default-Start:     s\n"),
            BinaryPackageCase::from(vec![
                ("etc/init.d/foo".to_string(), TestFile::new(rcs("foo"))),
                (
                    "usr/lib/systemd/system/foo.service".to_string(),
                    TestFile::new("[Service]\nExecStart=/bin/true\n"),
                ),
            ]),
            BinaryPackageCase::from(vec![
                (
                    "etc/init.d/anothertest".to_string(),
                    TestFile::new(rcs("anothertest")),
                ),
                (
                    "usr/lib/systemd/system/other.service".to_string(),
                    TestFile::new("[Install]\nAlias=anothertest\n"),
                ),
            ]),
            BinaryPackageCase::from(vec![
                ("etc/init.d/foo".to_string(), TestFile::new(rcs("foo"))),
                (
                    "usr/lib/systemd/system/foo@.service".to_string(),
                    TestFile::new("[Service]\nExecStart=/bin/true\n"),
                ),
            ]),
            BinaryPackageCase::from(vec![(
                "etc/init.d/bootmisc.sh".to_string(),
                TestFile::new(rcs("bootmisc")),
            )]),
            script("skeleton", &rcs("skeleton")),
            script("rc", &rcs("rc")),
            script("rcS", &rcs("rcS")),
            script("README", &rcs("README")),
            script("foo", &rcs("foo")).package("initscripts"),
            BinaryPackageCase::from(vec![
                (
                    "etc/init.d/masked".to_string(),
                    TestFile::new(rcs("masked")),
                ),
                (
                    "lib/systemd/system/masked.service".to_string(),
                    TestFile::symlink("/dev/null"),
                ),
            ]),
            BinaryPackageCase::from(vec![
                ("etc/init.d/foo.sh".to_string(), TestFile::new(rcs("foo"))),
                (
                    "usr/lib/systemd/system/foo.service".to_string(),
                    TestFile::new("[Service]\nExecStart=/bin/true\n"),
                ),
            ]),
            BinaryPackageCase::from(vec![(
                "etc/init.d/job".to_string(),
                TestFile::symlink("/lib/init/upstart-job"),
            )]),
        ];
        let fail = vec![
            script("boot", &rcs("boot")),
            BinaryPackageCase::from(vec![
                ("etc/init.d/foo".to_string(), TestFile::new(rcs("foo"))),
                (
                    "usr/lib/systemd/system/other.service".to_string(),
                    TestFile::new("[Service]\nExecStart=/bin/true\n"),
                ),
            ]),
            script("foo.sh", &rcs("foo")),
            script("udeb-boot", &rcs("udeb")).udeb(),
            script("early", &rcs("early")).package("sysvinit"),
            script("comment", "#!/bin/sh\necho # Default-Start: S\n"),
        ];
        BinaryPackageTester::new(MissingSystemdServiceForInitDRcSScript, pass, fail)
            .test_and_snapshot();
    }
}
