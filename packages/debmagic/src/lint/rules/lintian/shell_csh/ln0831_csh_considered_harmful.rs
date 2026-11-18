use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::interpreter::{self, Hashbang};
use crate::lint::rule::BinaryPackageRule;

pub struct CshConsideredHarmful;

declare_rule! {
    /// A script's interpreter is `csh` or `tcsh`.
    ///
    /// Regular Data files and Control files (maintainer scripts) are read for
    /// their hashbang. An interpreter whose basename is `csh` or `tcsh` is
    /// flagged at line 1, with the interpreter as Extra. Data files under
    /// `usr/share/doc/` (except `usr/share/doc/<package>/examples/`) and
    /// `usr/src/` are skipped, and so is `etc/csh/login.d/`. Control files are
    /// all checked. Udebs are checked too.
    CshConsideredHarmful,
    code = "LN0831",
    tag = "csh-considered-harmful",
    default_selected = true,
    experimental = false,
}

static EXAMPLES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/doc/[^/]+/examples/").expect("valid regex"));

impl BinaryPackageRule for CshConsideredHarmful {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let mut hits = Vec::new();
        if let Some(files) = ctx.data_files() {
            hits.extend(csh_scripts(files, true));
        }
        if let Some(files) = ctx.control_files() {
            hits.extend(csh_scripts(files, false));
        }
        for (path, interpreter) in hits {
            ctx.diagnostic(Diagnostic::warn(interpreter).with_location(Location {
                path,
                line: Some(1),
                column: None,
            }));
        }
    }
}

/// Regular files whose interpreter is `csh` or `tcsh`. `installed` selects
/// the Data files rules, which skip some trees.
fn csh_scripts(files: &FileTree, installed: bool) -> Vec<(PathBuf, String)> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .into_iter()
        .filter(|entry| !installed || !skipped(&entry.path().to_string_lossy()))
        .filter_map(|entry| {
            let bytes = files.read(entry).ok()?;
            let interpreter = Hashbang::read(&bytes)?.interpreter();
            let basename = interpreter::basename(&interpreter);
            matches!(basename, "csh" | "tcsh").then(|| (entry.path().to_path_buf(), interpreter))
        })
        .collect()
}

fn skipped(name: &str) -> bool {
    let documentation = name.starts_with("usr/share/doc/") && !EXAMPLES.is_match(name);
    documentation || name.starts_with("usr/src/") || name.starts_with("etc/csh/login.d/")
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content))])
    }

    fn control(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("usr/bin/tool", "#!/bin/sh\n"),
            script("usr/bin/tool", "#!/usr/bin/env python3\n"),
            // Other interpreters whose names start with csh or contain it.
            script("usr/bin/tool", "#!/bin/cshx\n"),
            script("usr/bin/tool", "#!/usr/bin/mycsh\n"),
            // Not a script.
            script("usr/bin/tool", "/bin/csh\n"),
            // Skipped trees: documentation and sources, and login.d.
            script("usr/share/doc/example/tool", "#!/bin/csh\n"),
            script("usr/share/doc/example/sub/examples/tool", "#!/bin/csh\n"),
            script("usr/src/example/tool", "#!/bin/csh\n"),
            script("etc/csh/login.d/example.csh", "#!/bin/csh\n"),
            // Maintainer scripts that are not csh.
            control("postinst", "#!/bin/sh\n"),
        ];
        let fail = vec![
            script("usr/bin/tool", "#!/bin/csh\n"),
            script("usr/bin/tool", "#!/usr/bin/env tcsh\n"),
            script("usr/bin/tool", "#!/bin/csh -f\n"),
            // Examples under documentation are checked.
            script("usr/share/doc/example/examples/tool", "#!/bin/csh\n"),
            script("etc/csh/other.csh", "#!/bin/csh\n"),
            script("usr/bin/tool", "#!/bin/csh\n").udeb(),
            // Maintainer scripts are checked.
            control("postinst", "#!/bin/csh\n"),
        ];
        BinaryPackageTester::new(CshConsideredHarmful, pass, fail).test_and_snapshot();
    }
}
