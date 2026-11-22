use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct MaintainerScriptCallsService;

declare_rule! {
    /// A shell maintainer script runs the `service` command.
    ///
    /// `service` is reserved for local administrators; packages use
    /// `invoke-rc.d` and `update-rc.d`. Only scripts whose shebang names a
    /// shell are scanned, at any line of the logical line, including inside a
    /// debhelper automatic section. The command is `service` followed by
    /// whitespace at a command position (`LEADING_PATTERN`); the Extra is the
    /// match with the leading command position trimmed, in double quotes, such
    /// as `"service"`. The Control file and line are the Location. Experimental,
    /// so not selected by default.
    MaintainerScriptCallsService,
    code = "LN1427",
    tag = "maintainer-script-calls-service",
    default_selected = false,
    experimental = true,
}

static COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&scan::leading(r"service\s+")).expect("valid regex"));

const KIND: CommandPattern = CommandPattern {
    shell_only: true,
    skip_automatic: false,
    quote: true,
};

impl BinaryPackageRule for MaintainerScriptCallsService {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for hit in scan::find_commands(files, &COMMAND, &KIND) {
            ctx.diagnostic(Diagnostic::error(hit.message).with_location(Location {
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
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Not a shell script.
            control("postinst", "#!/usr/bin/python3\nservice('x')\n"),
            // Not a maintainer script.
            BinaryPackageCase::from(vec![(
                "usr/share/app/script",
                TestFile::new("#!/bin/sh\nservice x start\n"),
            )]),
            // A word that is not the command.
            control("postinst", "#!/bin/sh\nmyservice x start\n"),
            control("postinst", "#!/bin/sh\necho services are fine\n"),
        ];
        let fail = vec![
            control("postinst", "#!/bin/sh\nservice x start\n"),
            control("prerm", "#!/bin/sh\nif service x status; then :; fi\n"),
            control("config", "#!/bin/bash\n  service x stop\n"),
        ];
        BinaryPackageTester::new(MaintainerScriptCallsService, pass, fail).test_and_snapshot();
    }
}
