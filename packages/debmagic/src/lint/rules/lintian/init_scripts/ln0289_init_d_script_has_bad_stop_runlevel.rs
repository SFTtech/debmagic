use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptHasBadStopRunlevel;

declare_rule! {
    /// An `etc/init.d` script lists a `Default-Stop` runlevel outside `s`,
    /// `S`, and `0`–`6`.
    ///
    /// The keyword is the last `Default-Stop` in the first LSB section. Each
    /// whitespace-separated token that is not one of those characters is its
    /// own finding. Extra is the token. The script path is the Location. The
    /// same scripts are skipped as for the required-option Tag.
    InitDScriptHasBadStopRunlevel,
    code = "LN0289",
    tag = "init.d-script-has-bad-stop-runlevel",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptHasBadStopRunlevel {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            for level in &script.lsb.default_stop {
                if !init_script::valid_runlevel(level) {
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

    fn lsb(stop: &str) -> String {
        format!(
            "#!/bin/sh\n### BEGIN INIT INFO\n# Default-Stop: {stop}\n### END INIT INFO\nexit 0\n"
        )
    }

    #[test]
    fn test() {
        let last_wins = "\
#!/bin/sh
### BEGIN INIT INFO
# Default-Stop: X
# Default-Stop: 0 1 6
### END INIT INFO
";
        let pass = vec![
            script("ok", &lsb("0 1 6")),
            script("letters", &lsb("S s")),
            script("last-wins", last_wins),
            script(
                "two-spaces",
                "#!/bin/sh\n### BEGIN INIT INFO\n#  Default-Stop: X\n### END INIT INFO\n",
            ),
            script("outside", "#!/bin/sh\n# Default-Stop: X\n"),
            script("skeleton", &lsb("X")),
            script("pkg", &lsb("X")).package("initscripts"),
        ];
        let fail = vec![
            script("letter", &lsb("S 0 1 6 X")),
            script("seven", &lsb("7")),
            script("pair", &lsb("00 Y")),
            script("y", &lsb("Y")),
            script("udeb-script", &lsb("Z")).udeb(),
        ];
        BinaryPackageTester::new(InitDScriptHasBadStopRunlevel, pass, fail).test_and_snapshot();
    }
}
