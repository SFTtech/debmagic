use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::rule::BinaryPackageRule;

use super::syntax::{Checker, failing_paths, host_checkers};

pub struct MaintainerShellScriptFailsSyntaxCheck;

declare_rule! {
    /// A maintainer script of the Binary package has a syntax error.
    ///
    /// Every Control file whose interpreter is `sh` or `bash` (after
    /// `/usr/bin/env`) is run through that shell with `-n`, as `/bin/dash -n`
    /// or `/bin/bash -n`. Reported when the shell exits non-zero. Scripts that
    /// re-exec or background themselves, symlinks outside the tree, and other
    /// interpreters are skipped. The Rule has no verdict, and reports nothing,
    /// when the checking shell is not installed. Extra is empty; the Control
    /// file name is the Location. Applies to udebs too.
    MaintainerShellScriptFailsSyntaxCheck,
    code = "LN1425",
    tag = "maintainer-shell-script-fails-syntax-check",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MaintainerShellScriptFailsSyntaxCheck {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let checkers = host_checkers();
        if checkers.is_empty() {
            return;
        }
        let Some(files) = ctx.control_files() else {
            return;
        };
        let failing = failing_scripts(files, &checkers);
        for path in failing {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Control files whose shell rejects them with `-n`.
fn failing_scripts(files: &FileTree, checkers: &[Checker]) -> Vec<std::path::PathBuf> {
    let candidates: Vec<&Entry> = files
        .entries()
        .iter()
        .filter(|entry| entry.is_file())
        .collect();
    failing_paths(files, &candidates, checkers)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn maintainer(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            maintainer("postinst", "#!/bin/sh\necho ok\n"),
            maintainer("postinst", "#!/usr/bin/env bash\nset -e\n[ -n x ]\n"),
            // Not a shell: lintian does not check it.
            maintainer("postinst", "#!/usr/bin/perl\nif (\n"),
            // No interpreter.
            maintainer("postinst", "if then\n"),
            // A wrapper that re-execs itself is skipped.
            maintainer("postinst", "#!/bin/sh\nexec \"$0\" \"$@\"\nif then\n"),
        ];
        let fail = vec![
            maintainer("postinst", "#!/bin/sh\nif true; then\n"),
            maintainer("prerm", "#!/bin/bash\ncase x in\n"),
            maintainer("config", "#!/usr/bin/env sh\nfi\n"),
        ];
        BinaryPackageTester::new(MaintainerShellScriptFailsSyntaxCheck, pass, fail).test();
    }
}
