use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct MaintainerScriptCallsInstallSgmlcatalog;

declare_rule! {
    /// A maintainer script runs `install-sgmlcatalog`.
    ///
    /// Any shebang interpreter is scanned, including inside a debhelper
    /// automatic section. Extra is `"install-sgmlcatalog"`. The Location is
    /// the Control file at the physical line where the logical line ends.
    MaintainerScriptCallsInstallSgmlcatalog,
    code = "LN0186",
    tag = "maintainer-script-calls-install-sgmlcatalog",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\binstall-sgmlcatalog\b").expect("valid regex"));

const KIND: CommandPattern = CommandPattern {
    shell_only: false,
    skip_automatic: false,
    quote: true,
};

impl BinaryPackageRule for MaintainerScriptCallsInstallSgmlcatalog {
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
            control("control", "Package: example\n"),
            control("postinst", "#!/bin/sh\nset -e\nupdate-foo\n"),
            control("postinst", "#!/bin/sh\n# install-sgmlcatalog --remove\n"),
            control("postinst", "install-sgmlcatalog --install package\n"),
            control("postinst", "#!\ninstall-sgmlcatalog\n"),
            control("triggers", "#!/bin/sh\ninstall-sgmlcatalog\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::symlink("foo")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::directory()),
        ];
        let fail = vec![
            control(
                "postinst",
                "#!/bin/sh\ninstall-sgmlcatalog --install package\ninstall-sgmlcatalog --remove package\n",
            ),
            control(
                "prerm",
                "#!/bin/sh\ninstall-sgmlcatalog --quiet --remove package\n",
            ),
            control(
                "postrm",
                "#!/bin/sh\ninstall-sgmlcatalog --remove package\n",
            ),
            control(
                "postinst",
                "#!/usr/bin/perl\ninstall-sgmlcatalog --remove package\n",
            ),
            control("postinst", "#!/usr/bin/env sh\ninstall-sgmlcatalog\n"),
            control(
                "postinst",
                "#!/bin/sh\ninstall-sgml\\\ncatalog --remove package\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\n# Automatically added by dh_dummy/12\ninstall-sgmlcatalog\n# End automatically added section\n",
            ),
            control("config.sh", "#!/bin/sh\ninstall-sgmlcatalog\n"),
        ];
        BinaryPackageTester::new(MaintainerScriptCallsInstallSgmlcatalog, pass, fail)
            .test_and_snapshot();
    }
}
