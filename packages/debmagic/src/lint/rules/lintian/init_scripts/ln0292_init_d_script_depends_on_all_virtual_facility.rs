use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptDependsOnAllVirtualFacility;

declare_rule! {
    /// An init script depends on the reserved `$all` virtual facility.
    ///
    /// `$all` is matched literally on `Required-Start`, `Should-Start`,
    /// `Required-Stop`, and `Should-Stop`, in that order, from the last
    /// assignment in the first LSB section. The virtual-facility table lists
    /// `$all` but is not consulted; `$ALL` and `$allfoo` do not match. Extra
    /// is the keyword. The script path is the Location. The same scripts are
    /// skipped as for the required-option Tag.
    InitDScriptDependsOnAllVirtualFacility,
    code = "LN0292",
    tag = "init.d-script-depends-on-all-virtual-facility",
    default_selected = true,
    experimental = false,
}

const KEYWORDS: &[&str] = &[
    "required-start",
    "should-start",
    "required-stop",
    "should-stop",
];

impl BinaryPackageRule for InitDScriptDependsOnAllVirtualFacility {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            for keyword in KEYWORDS {
                for facility in script.lsb.facilities(keyword) {
                    if facility == "$all" {
                        ctx.diagnostic(Diagnostic::error(*keyword).with_location(at(&script.path)));
                    }
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
        let last_wins = "\
#!/bin/sh
### BEGIN INIT INFO
# Required-Start: $all
# Required-Start: $local_fs
### END INIT INFO
";
        let second = "\
#!/bin/sh
### BEGIN INIT INFO
# Required-Start: $local_fs
### END INIT INFO
### BEGIN INIT INFO
# Required-Start: $all
### END INIT INFO
";
        let pass = vec![
            script("local", &lsb("# Required-Start: $local_fs\n")),
            script("almost", &lsb("# Required-Start: $ALL $allfoo all\n")),
            script("last-wins", last_wins),
            script("second-ignored", second),
            script("skeleton", &lsb("# Required-Stop: $all\n")),
            script("pkg", &lsb("# Required-Start: $all\n")).package("initscripts"),
        ];
        let four = lsb("# Required-Start: $all\n\
             # Should-Start: $all\n\
             # Required-Stop: $all\n\
             # Should-Stop: $all\n");
        let mixed = lsb("# Required-Start: networking $all mountall\n");
        let fail = vec![
            script("all-four", &four),
            script(
                "stop-only",
                &lsb("# Required-Stop: $all\n# Default-Stop:\n"),
            ),
            script("mixed", &mixed),
            script("udeb-script", &lsb("# Should-Start: $all\n")).udeb(),
        ];
        BinaryPackageTester::new(InitDScriptDependsOnAllVirtualFacility, pass, fail)
            .test_and_snapshot();
    }
}
