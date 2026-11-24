use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptMissingLsbSection;

declare_rule! {
    /// An `etc/init.d` script has no LSB header.
    ///
    /// The header is a line that starts with `### BEGIN INIT INFO`. The same
    /// scripts are skipped as for the required-option Tag. Extra is empty.
    /// The script path is the Location.
    InitDScriptMissingLsbSection,
    code = "LN0232",
    tag = "init.d-script-missing-lsb-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptMissingLsbSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let registered = {
            let Some(control) = ctx.control_files() else {
                return;
            };
            init_script::registered_names(control)
        };
        let Some(data) = ctx.data_files() else {
            return;
        };
        let scripts = init_script::init_d_scripts(data, &registered, &package);
        for script in scripts {
            if script.has_lsb {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn("").with_location(at(&script.path)));
        }
    }
}

fn at(path: &Path) -> Location {
    Location {
        path: path.to_path_buf(),
        line: None,
        column: None,
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
            script(
                "ok",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Default-Stop: 0 1 6\n### END INIT INFO\n",
            ),
            script("trailing", "#!/bin/sh\n### BEGIN INIT INFO extra\nexit 0\n"),
            script(
                "unterminated",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: foo\necho hi\n",
            ),
            script("skeleton", "#!/bin/sh\nexit 0\n"),
            script("bare", "#!/bin/sh\nexit 0\n").package("initscripts"),
            script("bare", "#!/bin/sh\nexit 0\n").package("sysvinit"),
            BinaryPackageCase::from(vec![(
                "etc/init.d/job".to_string(),
                TestFile::symlink("/lib/init/upstart-job"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/sbin/helper".to_string(),
                TestFile::new("#!/bin/sh\nexit 0\n"),
            )]),
        ];
        let fail = vec![
            script("no-lsb", "#!/bin/sh\nexit 0\n"),
            script("mid-line", "#!/bin/sh\necho ### BEGIN INIT INFO\n"),
            script("indented", "#!/bin/sh\n ### BEGIN INIT INFO\n"),
            BinaryPackageCase::from(vec![(
                "etc/init.d/hooked".to_string(),
                TestFile::new("#!/bin/sh\nexit 0\n").hardlink(),
            )]),
            script("skeleton", "#!/bin/sh\nexit 0\n").control_file(
                "postinst",
                TestFile::new("#!/bin/sh\nupdate-rc.d skeleton defaults\n"),
            ),
            script("udeb-script", "#!/bin/sh\nexit 0\n").udeb(),
        ];
        BinaryPackageTester::new(InitDScriptMissingLsbSection, pass, fail).test_and_snapshot();
    }
}
