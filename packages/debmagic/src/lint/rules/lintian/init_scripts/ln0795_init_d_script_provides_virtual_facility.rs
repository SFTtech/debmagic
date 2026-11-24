use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptProvidesVirtualFacility;

declare_rule! {
    /// An `etc/init.d` script lists a virtual facility, a name that starts with
    /// `$`, in `Provides`.
    ///
    /// The keyword is the last `Provides` in the first LSB section. Each
    /// whitespace-separated token that starts with `$` is its own finding.
    /// Extra is the token. The script path is the Location. The same scripts
    /// are skipped as for the required-option Tag.
    InitDScriptProvidesVirtualFacility,
    code = "LN0795",
    tag = "init.d-script-provides-virtual-facility",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptProvidesVirtualFacility {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            for facility in &script.lsb.provides {
                if facility.starts_with('$') {
                    ctx.diagnostic(
                        Diagnostic::warn(facility.clone()).with_location(at(&script.path)),
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

    fn lsb(provides: &str) -> String {
        format!(
            "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: {provides}\n### END INIT INFO\nexit 0\n"
        )
    }

    #[test]
    fn test() {
        let last_wins = "\
#!/bin/sh
### BEGIN INIT INFO
# Provides: $first
# Provides: plain
### END INIT INFO
";
        let pass = vec![
            script("plain", &lsb("plain")),
            script("dollar-inside", &lsb("a$b")),
            script("last-wins", last_wins),
            script("empty", &lsb("")),
            script("no-lsb", "#!/bin/sh\n# Provides: $outside\n"),
            script("pkg", &lsb("$x")).package("sysvinit"),
        ];
        let fail = vec![
            script("single", &lsb("$virtual")),
            script("several", &lsb("plain $one $two")),
            script("udeb-script", &lsb("$udeb")).udeb(),
        ];
        BinaryPackageTester::new(InitDScriptProvidesVirtualFacility, pass, fail)
            .test_and_snapshot();
    }
}
