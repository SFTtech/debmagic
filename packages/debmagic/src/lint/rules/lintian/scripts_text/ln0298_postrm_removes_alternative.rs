use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct PostrmRemovesAlternative;

declare_rule! {
    /// A `postrm` runs `update-alternatives --remove`.
    ///
    /// Any shebang interpreter is scanned. The Control file path is exactly
    /// `postrm`. Extra is the match in double quotes. `--remove-all` matches:
    /// the word boundary falls before the hyphen. A debhelper automatic
    /// section is still scanned.
    PostrmRemovesAlternative,
    code = "LN0298",
    tag = "postrm-removes-alternative",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bupdate-alternatives\s+--remove\b").expect("valid regex"));

const KIND: CommandPattern = CommandPattern {
    shell_only: false,
    skip_automatic: false,
    quote: true,
};

impl BinaryPackageRule for PostrmRemovesAlternative {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for hit in scan::find_commands(files, &COMMAND, &KIND) {
            if !is_postrm(&hit.path) {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn(hit.message).with_location(Location {
                path: hit.path,
                line: Some(hit.line),
                column: None,
            }));
        }
    }
}

fn is_postrm(path: &Path) -> bool {
    path.to_str() == Some("postrm")
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
            control("postrm", "#!/bin/sh\nset -e\n"),
            control(
                "prerm",
                "#!/bin/sh\nupdate-alternatives --remove dummy /usr/bin/dummy\n",
            ),
            control("postrm", "#!/bin/sh\n# update-alternatives --remove foo\n"),
            control("postrm", "#!/bin/sh\nupdate-alternatives --removeX foo\n"),
            control("postrm", "update-alternatives --remove foo\n"),
            control("my-postrm", "#!/bin/sh\nupdate-alternatives --remove foo\n"),
            control("triggers", "#!/bin/sh\nupdate-alternatives --remove foo\n"),
        ];
        let fail = vec![
            control("postrm", "#!/bin/sh\nupdate-alternatives --remove foo\n"),
            control(
                "postrm",
                "#!/bin/sh\nupdate-alternatives --remove dummy /usr/bin/dummy-alternative\n",
            ),
            control(
                "postrm",
                "#!/bin/sh\nupdate-alternatives --remove-all foo\n",
            ),
            control(
                "postrm",
                "#!/usr/bin/perl\nupdate-alternatives --remove foo\n",
            ),
            control(
                "postrm",
                "#!/bin/sh\n# Automatically added by dh_dummy/12\nupdate-alternatives --remove foo\n# End automatically added section\n",
            ),
            control(
                "postrm",
                "#!/usr/bin/env sh\nupdate-alternatives \\\n    --remove foo /usr/bin/foo\n",
            ),
        ];
        BinaryPackageTester::new(PostrmRemovesAlternative, pass, fail).test_and_snapshot();
    }
}
