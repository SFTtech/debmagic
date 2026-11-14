use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptStartsInStopRunlevel;

declare_rule! {
    /// An `etc/init.d` script lists runlevel 0 or 6 in LSB `Default-Start`.
    ///
    /// Tokens come from the last `Default-Start` in the first LSB section,
    /// in order, and only the exact tokens `0` and `6` match. Extra is that
    /// token. The script path is the Location. The same scripts are skipped
    /// as for the required-option Tag.
    InitDScriptStartsInStopRunlevel,
    code = "LN0294",
    tag = "init.d-script-starts-in-stop-runlevel",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptStartsInStopRunlevel {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            for level in &script.lsb.default_start {
                if level == "0" || level == "6" {
                    ctx.diagnostic(
                        Diagnostic::error(level.clone()).with_location(at(&script.path)),
                    );
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

    fn lsb(start: &str) -> String {
        format!(
            "#!/bin/sh\n### BEGIN INIT INFO\n# Default-Start: {start}\n### END INIT INFO\nexit 0\n"
        )
    }

    #[test]
    fn test() {
        let last_wins = "\
#!/bin/sh
### BEGIN INIT INFO
# Default-Start: 0
# Default-Start: 2 3 4 5
### END INIT INFO
";
        let pass = vec![
            script("ok", &lsb("2 3 4 5")),
            script("single", &lsb("S")),
            script("bad", &lsb("X")),
            script("last-wins", last_wins),
            script(
                "stop-only",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Default-Stop: 0 6\n### END INIT INFO\n",
            ),
            script("skeleton", &lsb("0")),
            script("pkg", &lsb("0")).package("sysvinit"),
        ];
        let fail = vec![
            script("zero", &lsb("0")),
            script("six", &lsb("6")),
            script("both", &lsb("0 6 X")),
            script("duplicate", &lsb("0 0")),
            script("udeb-script", &lsb("6")).udeb(),
        ];
        BinaryPackageTester::new(InitDScriptStartsInStopRunlevel, pass, fail).test_and_snapshot();
    }
}
