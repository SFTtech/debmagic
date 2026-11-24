use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::init_script;

pub struct InitDScriptMissingLsbShortDescription;

declare_rule! {
    /// An `etc/init.d` script has an LSB section, but the first section has no
    /// `Short-Description` keyword.
    ///
    /// The keyword counts as set when its line is present, even with an empty
    /// value. A script without a BEGIN marker is not checked, and the same
    /// scripts are skipped as for the required-option Tag. Extra is empty; the
    /// script is the Location. Severity is info and the Tag is not selected by
    /// default, as in lintian. `Description` has no finding of its own here.
    InitDScriptMissingLsbShortDescription,
    code = "LN1567",
    tag = "init.d-script-missing-lsb-short-description",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptMissingLsbShortDescription {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            if !script.section.begun {
                continue;
            }
            let set = script
                .section
                .keywords
                .iter()
                .any(|(_, name)| name == "short-description");
            if !set {
                ctx.diagnostic(
                    Diagnostic::warn("")
                        .with_location(at(&script.path))
                        .with_severity(Severity::Info),
                );
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

    #[test]
    fn test() {
        let pass = vec![
            script(
                "ok",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n# Short-Description: a\n### END INIT INFO\n",
            ),
            // The keyword is set with an empty value.
            script(
                "empty-value",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Short-Description:\n### END INIT INFO\n",
            ),
            script("no-lsb", "#!/bin/sh\n# Short-Description: a\n"),
            script("pkg", "#!/bin/sh\n### BEGIN INIT INFO\n### END INIT INFO\n")
                .package("sysvinit"),
        ];
        let fail = vec![
            script(
                "no-short",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n### END INIT INFO\n",
            ),
            // A Description does not stand in for Short-Description.
            script(
                "description",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Description: a\n### END INIT INFO\n",
            ),
            // Only the first section counts; a later one does not set it.
            script(
                "second-section",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n### END INIT INFO\n### BEGIN INIT INFO\n# Short-Description: a\n### END INIT INFO\n",
            ),
            // A keyword outside the section is not in it.
            script(
                "outside",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n### END INIT INFO\n# Short-Description: a\n",
            ),
            script(
                "udeb-script",
                "#!/bin/sh\n### BEGIN INIT INFO\n### END INIT INFO\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(InitDScriptMissingLsbShortDescription, pass, fail)
            .test_and_snapshot();
    }
}
