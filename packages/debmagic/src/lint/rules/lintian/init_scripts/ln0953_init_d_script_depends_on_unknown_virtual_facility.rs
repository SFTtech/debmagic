use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script::{self, VIRTUAL_FACILITIES};

pub struct InitDScriptDependsOnUnknownVirtualFacility;

declare_rule! {
    /// An `etc/init.d` script's LSB `Required-Start` or `Required-Stop`
    /// names a `$` virtual facility that is not in lintian's list.
    ///
    /// Only the first LSB section is read. Each unknown `$facility` in
    /// `Required-Start` is reported, then each one in `Required-Stop`, in the
    /// order written. `Should-Start` and `Should-Stop` are not checked, and
    /// names without `$` are not facilities. The scripts checked are the ones
    /// the init-d check reads (see `init.d-script-contains-skeleton-template-content`
    /// for which scripts those are). The extra is the facility and the Location
    /// is the script path, without a line.
    InitDScriptDependsOnUnknownVirtualFacility,
    code = "LN0953",
    tag = "init.d-script-depends-on-unknown-virtual-facility",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptDependsOnUnknownVirtualFacility {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let registered = ctx
            .control_files()
            .map(init_script::registered_names)
            .unwrap_or_default();
        let Some(data) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(PathBuf, String)> = init_script::init_d_scripts(data, &registered, &package)
            .into_iter()
            .flat_map(|script| {
                let path = script.path;
                script
                    .lsb
                    .required_start
                    .into_iter()
                    .chain(script.lsb.required_stop)
                    .filter(|facility| {
                        facility.starts_with('$')
                            && !VIRTUAL_FACILITIES.contains(&facility.as_str())
                    })
                    .map(|facility| (path.clone(), facility))
                    .collect::<Vec<_>>()
            })
            .collect();
        for (path, facility) in hits {
            ctx.diagnostic(Diagnostic::error(facility).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(header: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "etc/init.d/foo".to_string(),
            TestFile::new(format!(
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: foo\n{header}# Default-Start: 2\n# Default-Stop: 0 6\n### END INIT INFO\n"
            )),
        )])
    }

    #[test]
    fn binary() {
        let pass = vec![
            script("# Required-Start: $local_fs $network\n# Required-Stop: $remote_fs\n"),
            script("# Required-Start: $all\n"),
            script("# Required-Start: networking\n"),
            script("# Should-Start: $something_that_doesnt_exist\n# Should-Stop: $bogus\n"),
            script("# Required-Start: $local_fs\n").package("initscripts"),
        ];
        let fail = vec![
            script("# Required-Start: $something_that_doesnt_exist\n"),
            script("# Required-Stop: $bogus\n"),
            script("# Required-Start: $local_fs $bogus\n# Required-Stop: $other $network\n"),
            script("# Required-Start: $bogus\n").udeb(),
        ];
        BinaryPackageTester::new(InitDScriptDependsOnUnknownVirtualFacility, pass, fail)
            .test_and_snapshot();
    }
}
