use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MaintainerScriptHasInvalidUpdateInetdOptions;

declare_rule! {
    /// A maintainer script calls `update-inetd` with a bad option pair.
    ///
    /// The call is `update-inetd` or `/usr/sbin/update-inetd` where a shell
    /// command may start. Any shebang is scanned. `--pattern` together with
    /// `--add` reports `(--pattern with --add)`. `--group` without `--add`
    /// reports `(--group without --add)`. Both can match one line, pattern
    /// first. The options are substrings. A debhelper automatic section is
    /// still scanned.
    MaintainerScriptHasInvalidUpdateInetdOptions,
    code = "LN0302",
    tag = "maintainer-script-has-invalid-update-inetd-options",
    default_selected = true,
    experimental = false,
}

static CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&scan::leading(r"(?:/usr/sbin/)?update-inetd\s")).expect("valid regex")
});

impl BinaryPackageRule for MaintainerScriptHasInvalidUpdateInetdOptions {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let hits: Vec<_> = scan::script_lines(files)
            .into_iter()
            .filter(|line| CALL.is_match(&line.text))
            .flat_map(|line| {
                let mut messages = Vec::new();
                if line.text.contains("--pattern") && line.text.contains("--add") {
                    messages.push("(--pattern with --add)");
                }
                if line.text.contains("--group") && !line.text.contains("--add") {
                    messages.push("(--group without --add)");
                }
                let path = line.path;
                let number = line.line;
                messages
                    .into_iter()
                    .map(move |message| (path.clone(), number, message))
            })
            .collect();
        for (path, line, message) in hits {
            ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                path,
                line: Some(line),
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
            control("postinst", "#!/bin/sh\nupdate-inetd --add --group foo\n"),
            control("postinst", "#!/bin/sh\nupdate-inetd --group --add foo\n"),
            control("postinst", "#!/bin/sh\nupdate-inetd --pattern foo\n"),
            control("postinst", "#!/bin/sh\necho update-inetd --group foo\n"),
            control("postinst", "#!/bin/sh\n# update-inetd --group foo\n"),
            control("postinst", "update-inetd --group foo\n"),
            control("triggers", "#!/bin/sh\nupdate-inetd --group foo\n"),
        ];
        let fail = vec![
            control(
                "postinst",
                "#!/bin/sh\nupdate-inetd --group foo\nupdate-inetd --pattern --add foo\nupdate-inetd --add --pattern foo\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\n/usr/sbin/update-inetd --group foo\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\ntrue; update-inetd --group --pattern foo\n",
            ),
            control(
                "postinst",
                "#!/usr/bin/perl\nupdate-inetd --pattern --add foo\n",
            ),
            control(
                "prerm",
                "#!/bin/sh\n# Automatically added by dh_dummy/12\nupdate-inetd --group foo\n# End automatically added section\n",
            ),
        ];
        BinaryPackageTester::new(MaintainerScriptHasInvalidUpdateInetdOptions, pass, fail)
            .test_and_snapshot();
    }
}
