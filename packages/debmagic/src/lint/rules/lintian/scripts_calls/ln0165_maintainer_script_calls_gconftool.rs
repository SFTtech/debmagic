use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::script;

pub struct MaintainerScriptCallsGconftool;

declare_rule! {
    /// A maintainer script calls `gconftool` or `gconftool-2`.
    ///
    /// Same Control files as the dependency Tags: `config…`, `preinst`,
    /// `postinst`, `prerm`, and `postrm`, regular files with a shebang. The
    /// command is `gconftool` with an optional single-digit suffix and an
    /// optional `/usr/bin/` prefix, followed by whitespace or the end of the
    /// line. A package named `gconf` plus one digit (`gconf2`) is exempt.
    /// Extra is the matched command in double quotes. The Control file and
    /// line are the Location.
    MaintainerScriptCallsGconftool,
    code = "LN0165",
    tag = "maintainer-script-calls-gconftool",
    default_selected = true,
    experimental = false,
}

static CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:/usr/bin/)?gconftool(?:-[0-9])?(?:\s|\z)").expect("valid regex")
});
static EXEMPT_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^gconf[0-9]$").expect("valid regex"));

impl BinaryPackageRule for MaintainerScriptCallsGconftool {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if EXEMPT_PACKAGE.is_match(ctx.package_name()) {
            return;
        }
        let hits = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            script::maintainer_script_lines(files)
                .into_iter()
                .filter_map(|line| {
                    let command = CALL.find(&line.text)?.as_str().trim().to_string();
                    Some(script::CommandHit {
                        path: line.path,
                        line: line.line,
                        command,
                    })
                })
                .collect::<Vec<_>>()
        };
        for hit in hits {
            let message = format!("\"{}\"", hit.command);
            ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
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

    fn postinst(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("postinst", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            postinst("#!/bin/sh\nset -e\ntrue\n"),
            postinst("#!/bin/sh\ngconftool-2 --makefile-install-rule foo.schema\n")
                .package("gconf2"),
            postinst("#!/bin/sh\n# gconftool-2 --makefile-install-rule foo.schema\n"),
            postinst("#!/bin/sh\ngconftool-22 --foo\n"),
            postinst("gconftool-2 --foo\n"),
        ];
        let fail = vec![
            postinst("#!/bin/sh\ngconftool-2 --makefile-install-rule foo.schema\n"),
            postinst("#!/bin/sh\n/usr/bin/gconftool-2 --makefile-install-rule foo.schema\n"),
            postinst("#!/bin/sh\ngconftool --foo\n"),
            postinst("#!/usr/bin/perl\ngconftool-2 --foo\n"),
            postinst("#!/bin/sh\ngconftool-2 --foo\n").package("gconf"),
            postinst("#!/bin/sh\ngconftool-2 --foo\n").field("Depends", "gconf2"),
            postinst("#!/bin/sh\ngconftool-2 --foo\n").udeb(),
        ];
        BinaryPackageTester::new(MaintainerScriptCallsGconftool, pass, fail).test_and_snapshot();
    }
}
