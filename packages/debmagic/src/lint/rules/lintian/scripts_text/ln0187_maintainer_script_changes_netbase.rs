use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct MaintainerScriptChangesNetbase;

declare_rule! {
    /// A shell maintainer script rewrites a netbase file.
    ///
    /// `cp` or `mv` with `/etc/services`, `/etc/protocols`, or `/etc/rpc`
    /// after the source, or a `>` redirect onto one of those paths. The
    /// match may continue past a following space. Extra is the match in
    /// double quotes, with both ends trimmed. Other interpreters are not
    /// scanned.
    MaintainerScriptChangesNetbase,
    code = "LN0187",
    tag = "maintainer-script-changes-netbase",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\A\s*(?:cp|mv)\s+.*\s+|>\s*)/etc/(?:services|protocols|rpc)\s*(?:\s|\z)")
        .expect("valid regex")
});

const KIND: CommandPattern = CommandPattern {
    shell_only: true,
    skip_automatic: false,
    quote: true,
};

impl BinaryPackageRule for MaintainerScriptChangesNetbase {
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
                "#!/bin/sh\ncp /etc/protocols /etc/protocols.new\n",
            ),
            control("postinst", "#!/bin/sh\ncp /etc/services /tmp/out\n"),
            control("postinst", "#!/bin/sh\n# cp /nonexistent /etc/protocols\n"),
            control(
                "postinst",
                "#!/usr/bin/perl\ncp /nonexistent /etc/protocols\n",
            ),
            control("postinst", "cp /nonexistent /etc/protocols\n"),
            control("triggers", "#!/bin/sh\ncp /nonexistent /etc/protocols\n"),
        ];
        let fail = vec![
            control(
                "postinst",
                "#!/bin/sh\necho 'broken    6714/tcp' >> /etc/services\ncp /nonexistent /etc/protocols\nmv /usr/share/doc/rpc /etc/rpc\n",
            ),
            control("postinst", "#!/bin/sh\necho foo >/etc/rpc\n"),
            control("postinst", "#!/bin/sh\n  cp /tmp/x /etc/services extra\n"),
            control(
                "postinst",
                "#!/usr/bin/env bash\nmv /usr/share/doc/rpc /etc/rpc\n",
            ),
            control("my-postinst", "#!/bin/sh\ncp /nonexistent /etc/protocols\n"),
        ];
        BinaryPackageTester::new(MaintainerScriptChangesNetbase, pass, fail).test_and_snapshot();
    }
}
