use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptHasConflictingStartStop;

declare_rule! {
    /// An `etc/init.d` script lists one runlevel in both LSB `Default-Start`
    /// and `Default-Stop`.
    ///
    /// Only the first `### BEGIN INIT INFO` section is read. The last
    /// assignment of each keyword wins, and values are split on whitespace.
    /// Start tokens that are a single `s`, `S`, or `0`–`6` are stored
    /// lowercased, so `S` and `s` share the key `s`. The stop token is
    /// compared as written, so `Default-Stop: S` does not conflict with
    /// `Default-Start: S`. Extra is the stop token. The script path is the
    /// Location. The same scripts are skipped as for the required-option Tag.
    InitDScriptHasConflictingStartStop,
    code = "LN0288",
    tag = "init.d-script-has-conflicting-start-stop",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptHasConflictingStartStop {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            let starts = init_script::start_keys(&script.lsb.default_start);
            for level in &script.lsb.default_stop {
                if init_script::valid_runlevel(level) && starts.contains(level.as_str()) {
                    ctx.diagnostic(Diagnostic::warn(level.clone()).with_location(at(&script.path)));
                }
            }
        }
    }
}

fn scripts(ctx: &mut BinaryPackageContext<'_>) -> Vec<init_script::InitDScript> {
    let package = ctx.package_name().to_string();
    let registered = {
        let Some(control) = ctx.control_files() else {
            return Vec::new();
        };
        init_script::registered_names(control)
    };
    let Some(data) = ctx.data_files() else {
        return Vec::new();
    };
    init_script::init_d_scripts(data, &registered, &package)
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

    fn lsb(inner: &str) -> String {
        format!("#!/bin/sh\n### BEGIN INIT INFO\n{inner}\n### END INIT INFO\nexit 0\n")
    }

    #[test]
    fn test() {
        let second_ignored = "\
#!/bin/sh
### BEGIN INIT INFO
# Default-Start:     2 3 4 5
# Default-Stop:      0 1 6
### END INIT INFO
### BEGIN INIT INFO
# Default-Start:     0
# Default-Stop:      0
### END INIT INFO
";
        let pass = vec![
            script(
                "ok",
                &lsb("# Default-Start: 2 3 4 5\n# Default-Stop: 0 1 6"),
            ),
            script("capital-s", &lsb("# Default-Start: S\n# Default-Stop: S")),
            script(
                "start-s-stop-capital",
                &lsb("# Default-Start: s\n# Default-Stop: S"),
            ),
            script("two-spaces", &lsb("#  Default-Stop: 2\n# Default-Start: 2")),
            script("second-ignored", second_ignored),
            script(
                "outside",
                "#!/bin/sh\n# Default-Start: 2\n# Default-Stop: 2\n",
            ),
            script("skeleton", &lsb("# Default-Start: 1\n# Default-Stop: 1")),
            script("pkg", &lsb("# Default-Start: 1\n# Default-Stop: 1")).package("initscripts"),
            script("pkg", &lsb("# Default-Start: 1\n# Default-Stop: 1")).package("sysvinit"),
        ];
        let duplicate = lsb("# Default-Start: 1\n# Default-Stop: 1 1");
        let fail = vec![
            script(
                "digit",
                &lsb("# Default-Start: 1 2 3 4 5\n# Default-Stop: S 0 1 6 X"),
            ),
            script("lower-s", &lsb("# Default-Start: S\n# Default-Stop: s")),
            script(
                "both-digits",
                &lsb("# Default-Start: 0 6\n# Default-Stop: S Y 0 6"),
            ),
            script("duplicate", &duplicate),
            script("skeleton", &lsb("# Default-Start: 1\n# Default-Stop: 1")).control_file(
                "postinst",
                TestFile::new("#!/bin/sh\nupdate-rc.d skeleton defaults\n"),
            ),
            script("udeb-script", &lsb("# Default-Start: 2\n# Default-Stop: 2")).udeb(),
        ];
        BinaryPackageTester::new(InitDScriptHasConflictingStartStop, pass, fail)
            .test_and_snapshot();
    }
}
