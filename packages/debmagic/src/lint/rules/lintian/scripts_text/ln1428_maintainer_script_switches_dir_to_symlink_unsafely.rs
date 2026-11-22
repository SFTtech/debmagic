use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct MaintainerScriptSwitchesDirToSymlinkUnsafely;

declare_rule! {
    /// A maintainer script creates a symlink to a `doc` directory with
    /// `ln -s` or `ln -sf`, instead of `dpkg-maintscript-helper
    /// symlink_to_dir`.
    ///
    /// The match runs from `ln` to the last `doc` on the line. Extra is that
    /// text in double quotes. Every shebang interpreter is scanned, and
    /// automatically added sections are not skipped. Lines inside here
    /// documents are checked too.
    MaintainerScriptSwitchesDirToSymlinkUnsafely,
    code = "LN1428",
    tag = "maintainer-script-switches-dir-to-symlink-unsafely",
    default_selected = false,
    experimental = true,
}

static COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"ln[ ]*-f?s.*doc").expect("valid regex"));

const KIND: CommandPattern = CommandPattern {
    shell_only: false,
    skip_automatic: false,
    quote: true,
};

impl BinaryPackageRule for MaintainerScriptSwitchesDirToSymlinkUnsafely {
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
            control("postinst", "#!/bin/sh\nln -s foo bar\n"),
            control("postinst", "#!/bin/sh\n# ln -s ../share/doc /usr/doc\n"),
            control(
                "postinst",
                "#!/bin/sh\ndpkg-maintscript-helper symlink_to_dir /usr/doc /usr/share/doc 1.0\n",
            ),
            control("triggers", "#!/bin/sh\nln -sf ../share/doc /usr/doc\n"),
        ];
        let fail = vec![
            control(
                "postinst",
                "#!/bin/sh\nln -sf ../share/doc/maintainer-scripts /usr/doc\n",
            ),
            control("postinst", "#!/bin/sh\nln -s tworld-data /usr/share/doc\n"),
            control("postinst", "#!/usr/bin/perl\nln -s a /usr/doc\n"),
            control(
                "postinst",
                "#!/bin/sh\n# Automatically added by dh_dummy/12\nln -s a /usr/doc\n# End automatically added section\n",
            ),
            control("postinst", "#!/bin/sh\ncat <<EOF\nln -s a /usr/doc\nEOF\n"),
        ];
        BinaryPackageTester::new(MaintainerScriptSwitchesDirToSymlinkUnsafely, pass, fail)
            .test_and_snapshot();
    }
}
