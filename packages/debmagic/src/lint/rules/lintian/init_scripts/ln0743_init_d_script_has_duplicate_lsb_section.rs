use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptHasDuplicateLsbSection;

declare_rule! {
    /// An `etc/init.d` script has a second `### BEGIN INIT INFO` marker after
    /// its first LSB section ended.
    ///
    /// A marker inside the first section is not a second section. Each later
    /// marker is its own finding. Extra is empty. The Location is the script
    /// and the marker line. The same scripts are skipped as for the
    /// required-option Tag.
    InitDScriptHasDuplicateLsbSection,
    code = "LN0743",
    tag = "init.d-script-has-duplicate-lsb-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptHasDuplicateLsbSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            for line in &script.section.duplicate_sections {
                ctx.diagnostic(Diagnostic::error("").with_location(at(&script.path, *line)));
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

fn at(path: &Path, line: u32) -> Location {
    Location {
        path: path.to_path_buf(),
        line: Some(line),
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
            script(
                "ok",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n### END INIT INFO\n",
            ),
            script(
                "inner",
                "#!/bin/sh\n### BEGIN INIT INFO\n### BEGIN INIT INFO\n### END INIT INFO\n",
            ),
            script(
                "mid-line",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n### END INIT INFO\necho ### BEGIN INIT INFO\n",
            ),
            script(
                "skeleton",
                "#!/bin/sh\n### BEGIN INIT INFO\n### END INIT INFO\n### BEGIN INIT INFO\n",
            ),
            script(
                "pkg",
                "#!/bin/sh\n### BEGIN INIT INFO\n### END INIT INFO\n### BEGIN INIT INFO\n",
            )
            .package("sysvinit"),
        ];
        let fail = vec![
            script(
                "twice",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n### END INIT INFO\n### BEGIN INIT INFO\n### END INIT INFO\n",
            ),
            script(
                "after-unterminated",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n\n### BEGIN INIT INFO\n",
            ),
            script(
                "three",
                "#!/bin/sh\n### BEGIN INIT INFO\n### END INIT INFO\n### BEGIN INIT INFO\n### BEGIN INIT INFO\n",
            ),
            script(
                "udeb-script",
                "#!/bin/sh\n### BEGIN INIT INFO\n### END INIT INFO\n### BEGIN INIT INFO\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(InitDScriptHasDuplicateLsbSection, pass, fail).test_and_snapshot();
    }
}
