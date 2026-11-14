use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct MaintainerScriptCallsChownImproperly;

declare_rule! {
    /// A shell maintainer script runs `chown` with a dotted owner.
    ///
    /// `chown` plus optional short or long flags, then `user.group`, then
    /// whitespace. `user:group` does not match, and a dotted owner at the
    /// end of the line with no following space does not match. Extra is the
    /// `chown` invocation in double quotes, trailing space removed. Other
    /// interpreters are not scanned.
    MaintainerScriptCallsChownImproperly,
    code = "LN0189",
    tag = "maintainer-script-calls-chown-improperly",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"chown(?:\s+--?[A-Za-z-]+)*\s+[-_A-Za-z0-9]+\.[-_A-Za-z0-9]+\s+")
        .expect("valid regex")
});

const KIND: CommandPattern = CommandPattern {
    shell_only: true,
    skip_automatic: false,
    quote: true,
};

impl BinaryPackageRule for MaintainerScriptCallsChownImproperly {
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
                "#!/bin/sh\nchown root:root /usr/share/doc/maintainer-scripts/changelog\n",
            ),
            control("postinst", "#!/bin/sh\nchown root.root\n"),
            control("postinst", "#!/bin/sh\n# chown root.root /tmp\n"),
            control("postinst", "#!/usr/bin/perl\nchown root.root /tmp\n"),
            control("postinst", "chown root.root /tmp\n"),
            control("triggers", "#!/bin/sh\nchown -R root.root /tmp\n"),
        ];
        let fail = vec![
            control(
                "postinst",
                "#!/bin/sh\nchown root.root /usr/share/doc/maintainer-scripts/changelog\nchown -R root.root /usr/share/doc/maintainer-scripts\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\nchown --recursive root.root /usr/share/doc\n",
            ),
            control("postinst", "#!/usr/bin/env sh\nchown root.root /tmp\n"),
            control("postinst", "#!/bin/sh\n  chown root.root /tmp\n"),
        ];
        BinaryPackageTester::new(MaintainerScriptCallsChownImproperly, pass, fail)
            .test_and_snapshot();
    }
}
