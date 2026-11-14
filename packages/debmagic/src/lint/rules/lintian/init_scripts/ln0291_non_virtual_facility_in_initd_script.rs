use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct NonVirtualFacilityInInitdScript;

declare_rule! {
    /// An init script depends on a concrete facility that has a virtual name.
    ///
    /// The facilities are the last `Required-Start`, `Should-Start`,
    /// `Required-Stop`, and `Should-Stop` values in the first LSB section,
    /// in that keyword order. `mountall`, `mountnfs`, `hwclock`, `portmap`,
    /// `named`, `bind9`, `networking`, `syslog`, `rsyslog`, and `sysklogd`
    /// are the mapped names. `checkroot` is not. Extra is `facility -> $virtual`.
    /// The script path is the Location. The same scripts are skipped as for
    /// the required-option Tag.
    NonVirtualFacilityInInitdScript,
    code = "LN0291",
    tag = "non-virtual-facility-in-initd-script",
    default_selected = true,
    experimental = false,
}

const KEYWORDS: &[&str] = &[
    "required-start",
    "should-start",
    "required-stop",
    "should-stop",
];

impl BinaryPackageRule for NonVirtualFacilityInInitdScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            for keyword in KEYWORDS {
                for facility in script.lsb.facilities(keyword) {
                    let Some(replacement) = init_script::implied_facility(facility) else {
                        continue;
                    };
                    ctx.diagnostic(
                        Diagnostic::warn(format!("{facility} -> {replacement}"))
                            .with_location(at(&script.path)),
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

    fn lsb(inner: &str) -> String {
        format!("#!/bin/sh\n### BEGIN INIT INFO\n{inner}\n### END INIT INFO\nexit 0\n")
    }

    #[test]
    fn test() {
        let recipe = lsb("# Required-Start: mountall networking sysklogd\n\
             # Should-Start: portmap\n\
             # Required-Stop: mountnfs\n\
             # Default-Start: 0 6 X\n\
             # Default-Stop: S Y 0 6\n");
        let all_mapped = lsb(
            "# Required-Start: mountall mountnfs hwclock portmap named\n\
             # Should-Start: bind9 networking\n\
             # Required-Stop: syslog rsyslog\n\
             # Should-Stop: sysklogd\n",
        );
        let pass = vec![
            script(
                "virtual",
                &lsb("# Required-Start: $local_fs $network\n# Should-Start: checkroot cron\n"),
            ),
            script("empty", &lsb("# Required-Start:\n")),
            script("skeleton", &lsb("# Required-Start: networking\n")),
            script("pkg", &lsb("# Required-Start: networking\n")).package("sysvinit"),
        ];
        let fail = vec![
            script("init.d-lsb-headers", &recipe),
            script("mapped", &all_mapped),
            script("udeb-script", &lsb("# Should-Stop: named\n")).udeb(),
        ];
        BinaryPackageTester::new(NonVirtualFacilityInInitdScript, pass, fail).test_and_snapshot();
    }
}
