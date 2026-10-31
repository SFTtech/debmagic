use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptDoesNotImplementRequiredOption;

declare_rule! {
    /// An `etc/init.d` script does not implement a required action.
    ///
    /// Required actions are start, stop, restart, and force-reload, found in
    /// the script text the way the init.d check scans each line. A longer
    /// word at the same position wins, so `status` is not `start` and
    /// `restart` is not `start`. A shebang interpreter
    /// `/lib/init/init-d-script`, with an optional `/usr/bin/env`, or a line
    /// matching `. /lib/init/init-d-script`, counts as all of them. Comments
    /// hide the words. `initscripts` and `sysvinit` are skipped. `README`,
    /// `skeleton`, `rc`, and `rcS` are skipped unless `postinst` registers
    /// them with `update-rc.d`. Extra is each missing action, in that order.
    /// The script path is the Location.
    InitDScriptDoesNotImplementRequiredOption,
    code = "LN0230",
    tag = "init.d-script-does-not-implement-required-option",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptDoesNotImplementRequiredOption {
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
            for command in script.missing_commands {
                ctx.diagnostic(Diagnostic::error(command).with_location(at(&script.path)));
            }
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

    const COMPLETE: &str = "\
#!/bin/sh
### BEGIN INIT INFO
# Default-Start:     2 3 4 5
# Default-Stop:      0 1 6
### END INIT INFO
case \"$1\" in
    start) ;;
    stop) ;;
    restart|force-reload) ;;
esac
";

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("ok", COMPLETE),
            script("new", "#!/lib/init/init-d-script\nexit 0\n"),
            script("env", "#!/usr/bin/env /lib/init/init-d-script\nexit 0\n"),
            script("sourced", "#!/bin/sh\n. /lib/init/init-d-script\n"),
            script("skeleton", "#!/bin/sh\nexit 0\n"),
            script("bare", "#!/bin/sh\nexit 0\n").package("initscripts"),
            script("bare", "#!/bin/sh\nexit 0\n").package("sysvinit"),
            BinaryPackageCase::from(vec![(
                "etc/init.d/job".to_string(),
                TestFile::symlink("/lib/init/upstart-job"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/bin/not-init".to_string(),
                TestFile::new("#!/bin/sh\nexit 0\n"),
            )]),
        ];
        let commented = "\
#!/bin/sh
# start stop restart force-reload status
exit 0
";
        let status_only = "\
#!/bin/sh
case \"$1\" in
  status) ;;
esac
";
        let partial = "\
#!/bin/sh
case \"$1\" in
  start|stop) ;;
esac
";
        let fail = vec![
            script("partial", partial),
            script("commented", commented),
            script("status-only", status_only),
            BinaryPackageCase::from(vec![(
                "etc/init.d/hooked".to_string(),
                TestFile::new("#!/bin/sh\nexit 0\n").hardlink(),
            )]),
            script("udeb-script", "#!/bin/sh\nexit 0\n").udeb(),
            script("skeleton", "#!/bin/sh\nexit 0\n").control_file(
                "postinst",
                TestFile::new("#!/bin/sh\nupdate-rc.d skeleton defaults\n"),
            ),
        ];
        BinaryPackageTester::new(InitDScriptDoesNotImplementRequiredOption, pass, fail)
            .test_and_snapshot();
    }
}
