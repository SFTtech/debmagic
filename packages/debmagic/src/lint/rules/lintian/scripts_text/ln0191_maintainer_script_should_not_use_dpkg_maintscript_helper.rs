use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct MaintainerScriptShouldNotUseDpkgMaintscriptHelper;

declare_rule! {
    /// A maintainer script calls `dpkg-maintscript-helper` itself.
    ///
    /// Any shebang interpreter is scanned. The name matches as a substring.
    /// Lines between `# Automatically added by …` and
    /// `# End automatically added section` are skipped. A shell `\`
    /// continuation points at its last physical line. Extra is
    /// `"dpkg-maintscript-helper"`.
    MaintainerScriptShouldNotUseDpkgMaintscriptHelper,
    code = "LN0191",
    tag = "maintainer-script-should-not-use-dpkg-maintscript-helper",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"dpkg-maintscript-helper").expect("valid regex"));

const KIND: CommandPattern = CommandPattern {
    shell_only: false,
    skip_automatic: true,
    quote: true,
};

impl BinaryPackageRule for MaintainerScriptShouldNotUseDpkgMaintscriptHelper {
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
                "#!/bin/sh\n# detect usage that could be replaced by dpkg-maintscript-helper\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\n# Automatically added by dh_installdeb/13.11.4\ndpkg-maintscript-helper rm_conffile /etc/foo 1.0 -- \"$@\"\n# End automatically added section\n",
            ),
            control(
                "postinst",
                "dpkg-maintscript-helper symlink_to_dir /a /b 1.0\n",
            ),
            control(
                "triggers",
                "#!/bin/sh\ndpkg-maintscript-helper symlink_to_dir /a /b 1.0\n",
            ),
        ];
        let fail = vec![
            control(
                "preinst",
                "#!/bin/sh\ndpkg-maintscript-helper symlink_to_dir /path/name /old/target 1.0-1 pkg\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\ndpkg-maintscript-helper symlink_to_dir \\\n              /usr/share/autoconf-archive/html \\\n              ../../autoconf-archive/html \\\n               20111221-2~ -- \"$@\"\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\n# Automatically added by dh_dummy/12\ndpkg-maintscript-helper rm_conffile /etc/foo 1.0 -- \"$@\"\n# End automatically added section\ndpkg-maintscript-helper symlink_to_dir /a /b 1.0\n",
            ),
            control(
                "postinst",
                "#!/usr/bin/perl\necho dpkg-maintscript-helper\n",
            ),
            control(
                "postinst",
                "#!/usr/bin/env sh\ndpkg-maintscript-helper symlink_to_dir /a /b 1.0\n",
            ),
        ];
        BinaryPackageTester::new(
            MaintainerScriptShouldNotUseDpkgMaintscriptHelper,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
