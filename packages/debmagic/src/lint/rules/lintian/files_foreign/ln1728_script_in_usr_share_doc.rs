use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct ScriptInUsrShareDoc;

declare_rule! {
    /// An executable script is installed under `usr/share/doc/`.
    ///
    /// Walks Data files. A file or hardlink under `usr/share/doc/` with an
    /// execute bit set and a `#!` line (`#!#!#!` excluded) is reported. Files
    /// under an `examples/` directory (directly under the doc root or one
    /// package level down) are left out, as are executables without a `#!`
    /// line, which are `executable-in-usr-share-doc`. Extra is empty; the
    /// Location is the script. Udebs are checked too.
    ScriptInUsrShareDoc,
    code = "LN1728",
    tag = "script-in-usr-share-doc",
    default_selected = false,
    experimental = false,
}

static DOC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/doc/\S").expect("valid regex"));
static EXAMPLES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/doc/(?:[^/]+/)?examples/").expect("valid regex"));

impl BinaryPackageRule for ScriptInUsrShareDoc {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| is_candidate(entry) && is_script(files, entry))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

/// An executable file under `usr/share/doc/`, outside `examples/`.
fn is_candidate(entry: &Entry) -> bool {
    let Some(path) = entry.path().to_str() else {
        return false;
    };
    entry.is_executable_file() && DOC.is_match(path) && !EXAMPLES.is_match(path)
}

/// Content starts with a `#!` line. Unreadable content is not a script.
fn is_script(files: &FileTree, entry: &Entry) -> bool {
    files
        .read(entry)
        .is_ok_and(|bytes| Hashbang::read(&bytes).is_some())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn exec(content: &str) -> TestFile {
        TestFile::new(content).mode(0o755)
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(vec![]),
            // Examples are left out, at both depths.
            case(vec![(
                "usr/share/doc/foo/examples/run".to_string(),
                exec("#!/bin/sh\n"),
            )]),
            case(vec![(
                "usr/share/doc/examples/run".to_string(),
                exec("#!/bin/sh\n"),
            )]),
            // Not executable.
            case(vec![(
                "usr/share/doc/foo/run".to_string(),
                TestFile::new("#!/bin/sh\n").mode(0o644),
            )]),
            // An executable without a hashbang is another Tag.
            case(vec![("usr/share/doc/foo/data".to_string(), exec("binary"))]),
            // Lincity-style data is not a script.
            case(vec![(
                "usr/share/doc/foo/data".to_string(),
                exec("#!#!#!\n"),
            )]),
            // Outside usr/share/doc.
            case(vec![("usr/share/foo/run".to_string(), exec("#!/bin/sh\n"))]),
            // A symlink is not a file.
            case(vec![(
                "usr/share/doc/foo/run".to_string(),
                TestFile::symlink("/usr/bin/run"),
            )]),
        ];
        let fail = vec![
            case(vec![(
                "usr/share/doc/foo/run".to_string(),
                exec("#!/bin/sh\necho\n"),
            )]),
            // Any package directory, not only one level down.
            case(vec![(
                "usr/share/doc/foo/bar/run".to_string(),
                exec("#!/usr/bin/env python3\n"),
            )]),
            // Udebs are checked too.
            case(vec![(
                "usr/share/doc/foo/run".to_string(),
                exec("#!/bin/sh\n"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(ScriptInUsrShareDoc, pass, fail).test_and_snapshot();
    }
}
