use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct MaintainerScriptChangesLdSoConf;

declare_rule! {
    /// A shell maintainer script rewrites `/etc/ld.so.conf`.
    ///
    /// `cp` or `mv` with that path as the destination, or a `>` redirect onto
    /// it. Extra is the match in double quotes, with both ends trimmed. A
    /// package whose name matches `^libc` is exempt. Copying the file
    /// somewhere else does not match. Other interpreters are not scanned.
    MaintainerScriptChangesLdSoConf,
    code = "LN0304",
    tag = "maintainer-script-changes-ld-so-conf",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\A\s*(?:cp|mv)\s+.*\s+|>\s*)/etc/ld\.so\.conf\s*(?:\s|\z)")
        .expect("valid regex")
});
static EXCLUDE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^libc").expect("valid regex"));

const KIND: CommandPattern = CommandPattern {
    shell_only: true,
    skip_automatic: false,
    quote: true,
};

impl BinaryPackageRule for MaintainerScriptChangesLdSoConf {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if EXCLUDE.is_match(ctx.package_name()) {
            return;
        }
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

    const REWRITE: &str = "#!/bin/sh\necho '/usr/local/lib' >> /etc/ld.so.conf\nmv /etc/ld.so.conf.new /etc/ld.so.conf\n";

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            control("postinst", "#!/bin/sh\nset -e\n"),
            control(
                "postinst",
                "#!/bin/sh\ncp /etc/ld.so.conf /etc/ld.so.conf.bak\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\n( cat /etc/ld.so.conf ; echo '/usr/local/lib' ) > /etc/ld.so.conf.new\n",
            ),
            control("postinst", "#!/bin/sh\necho foo > /etc/ld.so.conf.bak\n"),
            control("postinst", "#!/bin/sh\n# echo foo >> /etc/ld.so.conf\n"),
            control("postinst", "#!/usr/bin/perl\necho foo >> /etc/ld.so.conf\n"),
            control("postinst", "echo foo >> /etc/ld.so.conf\n"),
            control("triggers", "#!/bin/sh\necho foo >> /etc/ld.so.conf\n"),
            control("postinst", REWRITE).package("libc6"),
            control("postinst", REWRITE).package("libc"),
        ];
        let fail = vec![
            control("postinst", REWRITE),
            control("postinst", "#!/bin/sh\n  cp /tmp/x /etc/ld.so.conf extra\n"),
            control(
                "postinst",
                "#!/usr/bin/env bash\nmv /etc/ld.so.conf.new /etc/ld.so.conf\n",
            ),
            control("postinst", REWRITE).package("not-libc"),
            control("my-postinst", "#!/bin/sh\necho x >/etc/ld.so.conf\n"),
        ];
        BinaryPackageTester::new(MaintainerScriptChangesLdSoConf, pass, fail).test_and_snapshot();
    }
}
