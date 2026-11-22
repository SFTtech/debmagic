use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::scan;

pub struct BashTermInPosixShell;

declare_rule! {
    /// An installed `/bin/sh` script uses a feature only bash has.
    ///
    /// Covers `==` in a test, `read` with an option other than `-r` or
    /// without a variable, `function`, `source`, `{a,b}`, `[[`, `time`,
    /// `echo -e`, `$RANDOM` and the other names in lintian's list. Every
    /// logical line is tested (continuations joined, comments removed), and
    /// the scan stops at an `exec` without a redirection. Extra is the match
    /// in single quotes, or `{hex:…}` for a control character.
    /// Only regular files and hardlinks are read; a symlink is skipped
    /// where lintian would follow one inside the tree.
    BashTermInPosixShell,
    code = "LN1430",
    tag = "bash-term-in-posix-shell",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for BashTermInPosixShell {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, finding) in installed_hits(files) {
            ctx.diagnostic(
                Diagnostic::warn(finding.extra)
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: Some(finding.line),
                        column: None,
                    }),
            );
        }
    }
}

/// Hints for every installed `sh` script, read before any diagnostic is
/// emitted so the file tree is not borrowed while the context is mutated.
fn installed_hits(files: &FileTree) -> Vec<(PathBuf, scan::Finding)> {
    let mut hits = Vec::new();
    for entry in files.entries() {
        if !entry.is_file() {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        if !scan::is_sh_script(&bytes) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        for finding in scan::findings(&text) {
            hits.push((entry.path().to_path_buf(), finding));
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn installed(content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/bin/example", TestFile::new(content))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            installed("#!/bin/sh\nset -e\necho hello\n"),
            installed("#!/bin/sh\n# echo -e is not used\n"),
            installed("#!/bin/sh\nread -r line\n"),
            installed("#!/bin/bash\nsource ./foo\n"),
            installed("#!/bin/sh\necho \"$HOME\" >/dev/null\n"),
            installed("#!/bin/sh\n[ -e foo ]\n"),
            installed("#!/bin/sh\necho 'x' && exec 3>&1\nexec foo\n"),
            installed("#!/usr/bin/perl\nsource x\n"),
            installed("#!/bin/sh\necho hi\nexec -l foo\necho $RANDOM\n"),
        ];
        let fail = vec![
            installed("#!/bin/sh\necho -e foo\n"),
            installed("#!/bin/sh\nsource ./foo\n"),
            installed("#!/bin/sh\nfunction foo {\n}\n"),
            installed("#!/bin/sh\nif [ \"$a\" == \"b\" ]; then true; fi\n"),
            installed("#!/bin/sh\necho $RANDOM\n"),
            installed("#!/bin/sh\nread\n"),
            installed("#!/bin/sh\necho hi\nexec\necho $RANDOM\n"),
            installed("#!/bin/sh\nsource $FOO\n"),
            installed("#!/bin/sh\ncat <<<\"x\"\n"),
        ];
        BinaryPackageTester::new(BashTermInPosixShell, pass, fail).test_and_snapshot();
    }
}
