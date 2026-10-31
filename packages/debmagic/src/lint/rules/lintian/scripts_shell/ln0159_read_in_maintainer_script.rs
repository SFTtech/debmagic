use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct ReadInMaintainerScript;

declare_rule! {
    /// A shell maintainer script reads an answer from the user.
    ///
    /// On a logical line (comments dropped, `\` continuations joined) that
    /// begins with `read`, unless the script has already mentioned
    /// `/usr/share/debconf/confmodule` on this line or an earlier one. A
    /// `read` inside a here-document is still reported. Extra is empty.
    /// Location is the Control file and the physical line where the logical
    /// line ends. Applies to udebs.
    ReadInMaintainerScript,
    code = "LN0159",
    tag = "read-in-maintainer-script",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ReadInMaintainerScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for script in scan::maintainer_scripts(files) {
            if !script.is_shell {
                continue;
            }
            let mut saw_debconf = false;
            for line in script.lines {
                if scan::line_mentions_debconf(&line.text) {
                    saw_debconf = true;
                }
                if saw_debconf || !scan::line_is_read(&line.text) {
                    continue;
                }
                ctx.diagnostic(Diagnostic::error("").with_location(Location {
                    path: script.path.clone(),
                    line: Some(line.number),
                    column: None,
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn with_script(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Empty and comment-only scripts.
            with_script("postinst", "#!/bin/sh\n"),
            with_script("postinst", "#!/bin/sh\n# read foo\n"),
            with_script("postinst", "#!/bin/sh\nset -e\nreadline foo\n"),
            with_script("postinst", "#!/bin/sh\nset -e\necho read foo\n"),
            // Debconf on an earlier line, or on the same line, suppresses `read`.
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\n. /usr/share/debconf/confmodule\nread foo\n",
            ),
            with_script(
                "postrm",
                "#!/bin/bash\nif ! test -f /usr/share/debconf/confmodule ; then\n    read\nfi\n",
            ),
            with_script(
                "postinst",
                "#!/bin/sh\nread foo; . /usr/share/debconf/confmodule\n",
            ),
            // Not scanned.
            with_script("postinst", "#!/usr/bin/env sh\nread foo\n"),
            with_script("postinst", "#!/usr/bin/perl\nread foo\n"),
            with_script("postinst", "read foo\n"),
            with_script("triggers", "#!/bin/sh\nread foo\n"),
            BinaryPackageCase::from(vec![(
                "usr/bin/example",
                TestFile::new("#!/bin/sh\nread foo\n"),
            )]),
        ];
        let fail = vec![
            with_script("postinst", "#!/bin/sh\nset -e\nread foo\n"),
            with_script("postinst", "#!/bin/sh\nset -e\nread\n"),
            with_script("prerm", "#!/bin/sh\nset -e\n  read -r foo\n"),
            // A here-document is not tracked, so the `read` is still reported.
            with_script("prerm", "#!/bin/sh\nset -e\ncat <<EOF\nread\nEOF\n"),
            // The `read` before debconf is reported; the one after is not.
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\nread early\n. /usr/share/debconf/confmodule\nread late\n",
            ),
            // Continuation: the report sits on the last physical line.
            with_script("postinst", "#!/bin/sh\nset -e\nread \\\nfoo\n"),
            with_script("postinst", "#! /bin/sh -e\nread\n"),
            with_script("postrm", "#!/usr/bin/env sh\nread foo\n"),
            with_script("postinst", "#!/bin/sh\nread foo\n").udeb(),
        ];
        BinaryPackageTester::new(ReadInMaintainerScript, pass, fail).test_and_snapshot();
    }
}
