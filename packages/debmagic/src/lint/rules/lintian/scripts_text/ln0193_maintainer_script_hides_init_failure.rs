use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct MaintainerScriptHidesInitFailure;

declare_rule! {
    /// A shell maintainer script hides an init-script failure.
    ///
    /// `invoke-rc.d` followed later by `||`, whitespace, `exit`, whitespace,
    /// and `0`. The dot in `invoke-rc.d` matches any character, as in
    /// lintian's pattern. Extra is the match in double quotes. A debhelper
    /// automatic section is still scanned. Other interpreters are not.
    MaintainerScriptHidesInitFailure,
    code = "LN0193",
    tag = "maintainer-script-hides-init-failure",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"invoke-rc.d.*\|\|\s+exit\s+0").expect("valid regex"));

const KIND: CommandPattern = CommandPattern {
    shell_only: true,
    skip_automatic: false,
    quote: true,
};

impl BinaryPackageRule for MaintainerScriptHidesInitFailure {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for hit in scan::find_commands(files, &COMMAND, &KIND) {
            ctx.diagnostic(Diagnostic::warn(hit.message).with_location(Location {
                path: hit.path,
                line: Some(hit.line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn control(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            control("postinst", "#!/bin/sh\nset -e\n"),
            control(
                "postinst",
                "#!/bin/sh\nif which invoke-rc.d >/dev/null 2>&1; then\n    invoke-rc.d package start\nfi\n",
            ),
            control("postinst", "#!/bin/sh\ninvoke-rc.d foo start || true\n"),
            control("postinst", "#!/bin/sh\ninvoke-rc.d foo start||exit 0\n"),
            control("postinst", "#!/bin/sh\n# invoke-rc.d foo start || exit 0\n"),
            control(
                "postinst",
                "#!/usr/bin/perl\ninvoke-rc.d foo start || exit 0\n",
            ),
            control("postinst", "invoke-rc.d foo start || exit 0\n"),
            control("triggers", "#!/bin/sh\ninvoke-rc.d foo start || exit 0\n"),
        ];
        let fail = vec![
            control("postinst", "#!/bin/sh\ninvoke-rc.d foo start || exit 0\n"),
            control(
                "postinst",
                "#!/bin/sh\n# Automatically added by dh_dummy/12\ninvoke-rc.d foo start || exit 0\n# End automatically added section\n",
            ),
            control(
                "postinst",
                "#!/usr/bin/env bash\ninvoke-rc.d foo start || exit 0\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\ninvoke-rc.d foo start || exit 0 || exit 0\n",
            ),
        ];
        BinaryPackageTester::new(MaintainerScriptHidesInitFailure, pass, fail).test_and_snapshot();
    }
}
