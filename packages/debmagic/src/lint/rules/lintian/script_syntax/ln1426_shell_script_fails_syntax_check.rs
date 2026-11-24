use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::rule::BinaryPackageRule;

use super::syntax::{Checker, failing_paths, host_checkers};

pub struct ShellScriptFailsSyntaxCheck;

declare_rule! {
    /// An installed script of the Binary package has a syntax error.
    ///
    /// Every installed file whose interpreter is `sh` or `bash` (after
    /// `/usr/bin/env`) is run through that shell with `-n`, as `/bin/dash -n`
    /// or `/bin/bash -n`. Reported when the shell exits non-zero. Skipped:
    /// paths under `usr/share/doc/` (examples are another Tag) and
    /// `usr/src/`, names ending in `.erb`, scripts that re-exec or background
    /// themselves, symlinks outside the tree, and other interpreters. The Rule
    /// has no verdict, and reports nothing, when the checking shell is not
    /// installed. Extra is empty; the path is the Location. Applies to udebs too.
    ShellScriptFailsSyntaxCheck,
    code = "LN1426",
    tag = "shell-script-fails-syntax-check",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ShellScriptFailsSyntaxCheck {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let checkers = host_checkers();
        if checkers.is_empty() {
            return;
        }
        let Some(files) = ctx.data_files() else {
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

/// Installed files whose shell rejects them with `-n`, outside the skipped
/// trees.
fn failing_scripts(files: &FileTree, checkers: &[Checker]) -> Vec<PathBuf> {
    let candidates: Vec<&Entry> = files
        .entries()
        .iter()
        .filter(|entry| !skipped(entry.path()))
        .collect();
    failing_paths(files, &candidates, checkers)
}

/// lintian's `visit_installed_files` skips: documentation (examples are a
/// different Tag), sources, and `.erb` templates.
fn skipped(path: &std::path::Path) -> bool {
    let name = path.to_string_lossy();
    name.starts_with("usr/share/doc/") || name.starts_with("usr/src/") || name.ends_with(".erb")
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content).mode(0o755))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("usr/bin/tool", "#!/bin/sh\necho ok\n"),
            script("usr/bin/tool", "#!/bin/bash\n[ -n x ] && echo ok\n"),
            // Other interpreters are not checked.
            script("usr/bin/tool", "#!/usr/bin/perl\nif (\n"),
            // Skipped trees and templates.
            script("usr/share/doc/pkg/examples/tool", "#!/bin/sh\nif then\n"),
            script("usr/share/doc/pkg/tool", "#!/bin/sh\nif then\n"),
            script("usr/src/pkg/tool", "#!/bin/sh\nif then\n"),
            script("usr/share/pkg/tool.erb", "#!/bin/sh\nif then\n"),
            // A wrapper that re-execs itself.
            script("usr/bin/tool", "#!/bin/sh\nexec \"$0\" \"$@\"\nif then\n"),
            // A symlink that leaves the tree.
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::symlink("../../../etc/passwd"),
            )]),
        ];
        let fail = vec![
            script("usr/bin/tool", "#!/bin/sh\nif true; then\n"),
            script("usr/lib/pkg/tool", "#!/bin/bash\ncase x in\n"),
            script("usr/bin/tool", "#!/usr/bin/env sh\nfi\n"),
            BinaryPackageCase::from(vec![
                (
                    "usr/bin/tool",
                    TestFile::new("#!/bin/sh\nif then\n").mode(0o755),
                ),
                ("usr/bin/alias", TestFile::symlink("tool")),
            ]),
        ];
        BinaryPackageTester::new(ShellScriptFailsSyntaxCheck, pass, fail).test();
    }
}
