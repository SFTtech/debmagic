use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::scan;

pub struct MaintainerScriptWithoutSetE;

declare_rule! {
    /// A shell maintainer script gets `-e` from its shebang but has no
    /// `set -e` in its body.
    ///
    /// The shebang carries an `e` flag (`#!/bin/sh -e`) and no logical line
    /// runs `set` with an `e` flag. The script then fails to abort when it
    /// is run by hand with `sh script`. Extra is empty. Location is the
    /// Control file name, with no line. Severity is pedantic. Applies to
    /// udebs.
    MaintainerScriptWithoutSetE,
    code = "LN1429",
    tag = "maintainer-script-without-set-e",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for MaintainerScriptWithoutSetE {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for script in scan::maintainer_scripts(files) {
            if script.is_shell && script.saw_bange && !script.saw_sete {
                ctx.diagnostic(
                    Diagnostic::warn("")
                        .with_severity(Severity::Pedantic)
                        .with_location(Location {
                            path: script.path,
                            line: None,
                            column: None,
                        }),
                );
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
            // No `e` flag on the shebang, so nothing to repeat in the body.
            with_script("postinst", "#!/bin/sh\n"),
            with_script("postinst", "#!/bin/sh\nset -e\n"),
            // `set -e` in the body satisfies the Tag.
            with_script("postinst", "#!/bin/sh -e\nset -e\n"),
            with_script("postinst", "#!/bin/sh -e\nset -eu\n"),
            with_script("postinst", "#!/bin/sh -e\nset -o errexit\n"),
            // Not a shell, so the shebang flag does not apply.
            with_script("postinst", "#!/usr/bin/perl -e\nprint 1;\n"),
            // Not a maintainer script.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("triggers", TestFile::new("#!/bin/sh -e\n")),
        ];
        let fail = vec![
            with_script("postinst", "#!/bin/sh -e\n"),
            with_script("postrm", "#!/bin/bash -eu\necho hi\n"),
            with_script("postinst", "#! /bin/sh -e\nread foo\n"),
            // A commented-out `set -e` does not count.
            with_script("preinst", "#!/bin/dash -e\n#set -e\n"),
            // The udeb case is the same.
            with_script("postinst", "#!/bin/sh -e\n").udeb(),
        ];
        BinaryPackageTester::new(MaintainerScriptWithoutSetE, pass, fail).test_and_snapshot();
    }
}
