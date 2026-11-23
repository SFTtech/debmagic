use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::syntax::{Checker, fails_syntax_check, host_checkers};

pub struct ExampleShellScriptFailsSyntaxCheck;

declare_rule! {
    /// An example script under `usr/share/doc/<package>/examples/` has a
    /// syntax error.
    ///
    /// The example variant of `shell-script-fails-syntax-check`. Installed
    /// files under the examples directory whose interpreter is `sh` or `bash`
    /// are run through `/bin/dash -n` or `/bin/bash -n`, and reported when the
    /// shell exits non-zero. Skipped: other documentation, `usr/src/`, names
    /// ending in `.erb`, scripts that re-exec or background themselves,
    /// symlinks outside the tree, and other interpreters. Reports nothing when
    /// the checking shell is not installed on the host. Extra is empty; the
    /// path is the Location. Udebs are checked too.
    ExampleShellScriptFailsSyntaxCheck,
    code = "LN1742",
    tag = "example-shell-script-fails-syntax-check",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for ExampleShellScriptFailsSyntaxCheck {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let checkers = host_checkers();
        if checkers.is_empty() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in failing_examples(files, &checkers) {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

/// Installed example files whose shell rejects them with `-n`.
fn failing_examples(files: &FileTree, checkers: &[Checker]) -> Vec<PathBuf> {
    files
        .entries()
        .iter()
        .filter(|entry| is_example(entry.path()))
        .filter(|entry| fails_syntax_check(files, entry, checkers))
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

/// `usr/share/doc/<package>/examples/…`, and not an `.erb` template.
fn is_example(path: &std::path::Path) -> bool {
    let name = path.to_string_lossy();
    let is_examples = name
        .strip_prefix("usr/share/doc/")
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(package, after)| !package.is_empty() && after.starts_with("examples/"));
    is_examples && !name.ends_with(".erb")
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
            script("usr/share/doc/pkg/examples/tool", "#!/bin/sh\necho ok\n"),
            // Other interpreters are not checked.
            script("usr/share/doc/pkg/examples/tool", "#!/usr/bin/perl\nif (\n"),
            // Not examples: the non-example Tag.
            script("usr/share/doc/pkg/tool", "#!/bin/sh\nif then\n"),
            script("usr/bin/tool", "#!/bin/sh\nif then\n"),
            // Skipped sources and templates.
            script("usr/src/pkg/examples/tool", "#!/bin/sh\nif then\n"),
            script(
                "usr/share/doc/pkg/examples/tool.erb",
                "#!/bin/sh\nif then\n",
            ),
            // A wrapper that re-execs itself.
            script(
                "usr/share/doc/pkg/examples/tool",
                "#!/bin/sh\nexec \"$0\" \"$@\"\nif then\n",
            ),
            // A symlink that leaves the tree.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/pkg/examples/tool",
                TestFile::symlink("../../../../etc/passwd"),
            )]),
        ];
        let fail = vec![
            script(
                "usr/share/doc/pkg/examples/syntax",
                "#!/bin/sh\nif true; then\n",
            ),
            script(
                "usr/share/doc/pkg/examples/tool",
                "#!/bin/bash\ncase x in\n",
            ),
            script("usr/share/doc/pkg/examples/tool", "#!/usr/bin/env sh\nfi\n").udeb(),
        ];
        BinaryPackageTester::new(ExampleShellScriptFailsSyntaxCheck, pass, fail)
            .test_and_snapshot();
    }
}
