use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MaintainerScriptIgnoresErrors;

declare_rule! {
    /// A shell maintainer script does not abort when a command fails.
    ///
    /// The Control file is a maintainer script (`config…`, or a name ending in
    /// `preinst`, `postinst`, `prerm`, or `postrm`) and a regular file whose
    /// shebang lintian accepts. It is a shell script, the shebang has no `e`
    /// flag, and no logical line runs `set` with an `e` flag (`set -e`,
    /// `set -eu`, `set -- -e`). `set -o errexit` does not count. Comment-only
    /// and empty shell scripts are included. Extra is empty. Location is the
    /// Control file name, with no line. Applies to udebs.
    MaintainerScriptIgnoresErrors,
    code = "LN0158",
    tag = "maintainer-script-ignores-errors",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MaintainerScriptIgnoresErrors {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for script in scan::maintainer_scripts(files) {
            if script.is_shell && !script.saw_sete && !script.saw_bange {
                ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                    path: script.path,
                    line: None,
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
            // `set -e` aborts on failure.
            with_script("postinst", "#!/bin/sh\nset -e\n"),
            with_script("postinst", "#!/bin/sh\nset -eu\n"),
            with_script("postinst", "#!/bin/sh\nfoo; set -e\n"),
            // The shebang itself carries `-e` (`maintainer-script-without-set-e`).
            with_script("postinst", "#!/bin/sh -e\n"),
            with_script("postinst", "#! /bin/sh -e\nread foo\n"),
            with_script("postrm", "#!/bin/sh -e\n"),
            // A joined continuation still counts.
            with_script("postinst", "#!/bin/sh\nset \\\n -e\n"),
            // Not a shell, or not scanned.
            with_script("postinst", "#!/usr/bin/perl\n"),
            with_script("postinst", "#!/usr/bin/env sh\n"),
            with_script("postinst", "#!/bin/pdksh\n"),
            with_script("postinst", "set -e\n"),
            with_script("postinst", ""),
            with_script("triggers", "#!/bin/sh\n"),
            // An installed script is not a Control file.
            BinaryPackageCase::from(vec![("usr/bin/example", TestFile::new("#!/bin/sh\n"))]),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::symlink("foo")),
            // `#set -e` is a comment, but this one has a real `set -e` too.
            with_script("postinst", "#!/bin/sh\n#set -e\nset -e\n"),
        ];
        let fail = vec![
            // Empty shell script.
            with_script("postinst", "#!/bin/sh\n"),
            with_script("preinst", "#!/bin/sh\n"),
            // Comment-only.
            with_script("postrm", "#!/bin/sh\n# we do not set -e\n"),
            with_script("postrm", "#!/bin/bash\n#set -e\n"),
            // `set -o errexit` is not recognized (lintian 2.139).
            with_script("prerm", "#!/bin/sh\nset -o errexit\n"),
            with_script("postinst", "#!/bin/dash\n"),
            with_script("config", "#!/bin/sh\n# nothing\n"),
            // `env` is not a known interpreter on postinst, but postrm is always scanned.
            with_script("postrm", "#!/usr/bin/env sh\n"),
            with_script("my-postinst", "#!/bin/sh\n"),
            with_script("postinst", "#!/bin/sh\n").udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("preinst", TestFile::new("#!/bin/sh\n"))
                .control_file("postrm", TestFile::new("#!/bin/sh\n# comment\n")),
        ];
        BinaryPackageTester::new(MaintainerScriptIgnoresErrors, pass, fail).test_and_snapshot();
    }
}
