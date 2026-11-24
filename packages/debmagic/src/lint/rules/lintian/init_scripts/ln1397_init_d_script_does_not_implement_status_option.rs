use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::init_script;

pub struct InitDScriptDoesNotImplementStatusOption;

declare_rule! {
    /// An `etc/init.d` script does not implement the `status` action. Reported
    /// at info severity, with an empty extra and the script path as the
    /// Location.
    ///
    /// The `status` word is found the way `init.d-script-does-not-implement-
    /// required-option` (LN0230) finds the required actions, and the same
    /// skips apply: `initscripts` and `sysvinit` are skipped, and `README`,
    /// `skeleton`, `rc`, and `rcS` are skipped unless `postinst` registers them.
    /// A shebang of `/lib/init/init-d-script`, or a line sourcing it, counts as
    /// implementing `status`.
    InitDScriptDoesNotImplementStatusOption,
    code = "LN1397",
    tag = "init.d-script-does-not-implement-status-option",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptDoesNotImplementStatusOption {
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
        let paths: Vec<_> = init_script::init_d_scripts(data, &registered, &package)
            .into_iter()
            .filter(|script| !init_script::has_status_command(&script.text))
            .map(|script| script.path)
            .collect();
        for path in paths {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(at(&path)),
            );
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
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script(
                "example",
                "#!/bin/sh\ncase \"$1\" in\n start|stop|restart|force-reload|status) ;;\nesac\n",
            ),
            script("example", "#!/bin/sh\ncase \"$1\" in status) ;; esac\n"),
            // Sourcing init-d-script implements every command.
            script("example", "#!/bin/sh\n. /lib/init/init-d-script\n"),
            script("example", "#!/lib/init/init-d-script\n"),
            // Skipped packages and special names.
            BinaryPackageCase::from(vec![("etc/init.d/example", TestFile::new("exit 0\n"))])
                .package("sysvinit"),
            script("skeleton", "#!/bin/sh\nexit 0\n"),
            script("README", "Not a script.\n"),
            // Not registered by postinst, so `rcS` is skipped.
            script("rcS", "#!/bin/sh\nexit 0\n"),
        ];
        let fail = vec![
            script("example", "#!/bin/sh\ncase \"$1\" in\n start) ;;\nesac\n"),
            script("example", "#!/bin/sh\nexit 0\n"),
            // A comment does not count.
            script(
                "example",
                "#!/bin/sh\n# status\nstart) ;; stop) ;; restart) ;; force-reload) ;;\n",
            ),
            // Reported for a udeb too.
            script("example", "#!/bin/sh\nexit 0\n").udeb(),
        ];
        BinaryPackageTester::new(InitDScriptDoesNotImplementStatusOption, pass, fail)
            .test_and_snapshot();
    }
}
