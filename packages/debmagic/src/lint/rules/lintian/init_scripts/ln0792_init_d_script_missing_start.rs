use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptMissingStart;

declare_rule! {
    /// An `etc/init.d` script starts in some of runlevels 2 to 5, but not in
    /// all of them.
    ///
    /// The keyword is the last `Default-Start` in the first LSB section. Only
    /// valid tokens count (`s`, `S`, and `0`–`6`). The finding is skipped when
    /// no runlevel from 2 to 5 is set, or when all four are set. Extra is the
    /// missing levels, in order, separated by spaces. The script path is the
    /// Location. The same scripts are skipped as for the required-option Tag.
    InitDScriptMissingStart,
    code = "LN0792",
    tag = "init.d-script-missing-start",
    default_selected = true,
    experimental = false,
}

const SERVICE_RUNLEVELS: [&str; 4] = ["2", "3", "4", "5"];

impl BinaryPackageRule for InitDScriptMissingStart {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            let starts = init_script::start_keys(&script.lsb.default_start);
            if !SERVICE_RUNLEVELS
                .iter()
                .any(|level| starts.contains(*level))
            {
                continue;
            }
            let missing: Vec<&str> = SERVICE_RUNLEVELS
                .into_iter()
                .filter(|level| !starts.contains(*level))
                .collect();
            if missing.is_empty() {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn(missing.join(" ")).with_location(at(&script.path)));
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
        let pass = vec![
            script("all", &lsb("2 3 4 5")),
            script("with-s", &lsb("s 2 3 4 5")),
            script("none", &lsb("0 6 S")),
            script("empty", &lsb("")),
            script("invalid-token", &lsb("X 2 3 4 5")),
            script("no-lsb", "#!/bin/sh\nexit 0\n"),
            script("pkg", &lsb("2")).package("sysvinit"),
        ];
        let fail = vec![
            script("three", &lsb("2 3 4")),
            script("two", &lsb("2 5")),
            script("one", &lsb("3")),
            script("invalid-then-two", &lsb("7 2")),
            script("udeb-script", &lsb("2 3")).udeb(),
        ];
        BinaryPackageTester::new(InitDScriptMissingStart, pass, fail).test_and_snapshot();
    }
}
