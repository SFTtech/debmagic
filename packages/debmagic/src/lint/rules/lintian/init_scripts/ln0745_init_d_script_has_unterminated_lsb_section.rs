use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptHasUnterminatedLsbSection;

declare_rule! {
    /// An `etc/init.d` script has a first LSB section that a non-comment line
    /// ends before its `### END INIT INFO` marker.
    ///
    /// The Location is the first line that is not a comment. A section that
    /// reaches the end of the file without a non-comment line is not reported.
    /// Extra is empty. The same scripts are skipped as for the required-option
    /// Tag.
    InitDScriptHasUnterminatedLsbSection,
    code = "LN0745",
    tag = "init.d-script-has-unterminated-lsb-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptHasUnterminatedLsbSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            if let Some(line) = script.section.unterminated {
                ctx.diagnostic(Diagnostic::error("").with_location(at(&script.path, line)));
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
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n### END INIT INFO\nexit 0\n",
            ),
            script("no-lsb", "#!/bin/sh\nexit 0\n"),
            script(
                "eof",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n# Description: b\n",
            ),
            script(
                "skeleton",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n\n",
            ),
            script(
                "pkg",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\nexit 0\n",
            )
            .package("initscripts"),
        ];
        let fail = vec![
            script(
                "blank",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n\n### END INIT INFO\n",
            ),
            script(
                "code",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\nexit 0\n### END INIT INFO\n",
            ),
            script(
                "after-bad-line",
                "#!/bin/sh\n### BEGIN INIT INFO\n#  Provides: a\n# Provides: b\n\n# Provides: c\n",
            ),
            script(
                "udeb-script",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\necho\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(InitDScriptHasUnterminatedLsbSection, pass, fail)
            .test_and_snapshot();
    }
}
