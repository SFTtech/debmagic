use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct InstallInfoUsedInMaintainerScript;

declare_rule! {
    /// A maintainer script runs `install-info`.
    ///
    /// Any shebang interpreter is scanned. Extra is `"install-info"`. A
    /// shell `\` continuation points at its last physical line. A `\`
    /// followed by a comment does not continue, so the Location stays on
    /// the line that still holds the command.
    InstallInfoUsedInMaintainerScript,
    code = "LN0192",
    tag = "install-info-used-in-maintainer-script",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\binstall-info\b").expect("valid regex"));

const KIND: CommandPattern = CommandPattern {
    shell_only: false,
    skip_automatic: false,
    quote: true,
};

impl BinaryPackageRule for InstallInfoUsedInMaintainerScript {
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
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            control("postinst", "#!/bin/sh\nset -e\n"),
            control(
                "postinst",
                "#!/bin/sh\n# install-info is now handled through triggers.\n",
            ),
            control("postinst", "#!/bin/install-info\nset -e\n"),
            control("postinst", "install-info --quiet\n"),
            control("triggers", "#!/bin/sh\ninstall-info --quiet\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::directory()),
        ];
        let fail = vec![
            control(
                "postinst",
                "#!/bin/sh\ninstall-info --quiet --section Development Development \\\n    /usr/share/info/foobar.info\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\ninstall-info /usr/share/info/foo \\\n\t--quiet \\ # make it so\n\t--section foo\n",
            ),
            control("postinst", "#!/usr/bin/perl\ninstall-info --quiet\n"),
            control("postinst", "#!/usr/bin/env sh\ninstall-info --quiet\n"),
            control(
                "postinst",
                "#!/bin/sh\necho \"install-info # not a comment\"\n",
            ),
        ];
        BinaryPackageTester::new(InstallInfoUsedInMaintainerScript, pass, fail).test_and_snapshot();
    }
}
