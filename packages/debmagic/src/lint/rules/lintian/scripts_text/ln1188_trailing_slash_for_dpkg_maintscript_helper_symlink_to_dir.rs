use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct TrailingSlashForDpkgMaintscriptHelperSymlinkToDir;

declare_rule! {
    /// A maintainer script passes a trailing slash to `symlink_to_dir`.
    ///
    /// `dpkg-maintscript-helper symlink_to_dir` followed by a path that ends
    /// in `/` and then whitespace. Such a call makes the package
    /// uninstallable. Any shebang is scanned, and heredoc bodies are scanned
    /// too. A shell `\` continuation is joined and the Location is its last
    /// physical line. Extra is the match, trimmed, in double quotes.
    TrailingSlashForDpkgMaintscriptHelperSymlinkToDir,
    code = "LN1188",
    tag = "trailing-slash-for-dpkg-maintscript-helper-symlink_to_dir",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"dpkg-maintscript-helper\s+symlink_to_dir\s+[^\s]+/\s").expect("valid regex")
});

const KIND: CommandPattern = CommandPattern {
    shell_only: false,
    skip_automatic: false,
    quote: true,
};

impl BinaryPackageRule for TrailingSlashForDpkgMaintscriptHelperSymlinkToDir {
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
            // No trailing slash, or the path ends at the end of the line: the
            // line is chomped, so no whitespace follows the slash.
            control(
                "postinst",
                "#!/bin/sh\ndpkg-maintscript-helper symlink_to_dir /usr/share/foo /usr/share/bar 1.0\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\ndpkg-maintscript-helper symlink_to_dir /usr/share/foo /usr/share/bar/\n",
            ),
            // Other helper commands.
            control(
                "postinst",
                "#!/bin/sh\ndpkg-maintscript-helper rm_conffile /etc/foo/ 1.0\n",
            ),
        ];
        let fail = vec![
            control(
                "postinst",
                "#!/bin/sh\ndpkg-maintscript-helper symlink_to_dir /usr/share/foo/ /usr/share/bar 1.0\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\ndpkg-maintscript-helper symlink_to_dir               /usr/share/autoconf-archive/html/ ../autoconf-archive/html\n",
            ),
            control(
                "prerm",
                "#!/bin/sh\ndpkg-maintscript-helper symlink_to_dir /usr/share/foo/ /usr/share/bar 1.0\n",
            ),
        ];
        BinaryPackageTester::new(
            TrailingSlashForDpkgMaintscriptHelperSymlinkToDir,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
