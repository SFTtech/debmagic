use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct ChownWithDot;

declare_rule! {
    /// A script calls `chown owner.group`, the deprecated dot separator.
    ///
    /// Checks Data files and Control files that are files or hardlinks and
    /// start with a `#!` line (`#!#!#!` excluded). Each line that does not
    /// start with `#` and is not empty is searched for `chown`, a word
    /// boundary, then any `-option` words, then a word containing a dot
    /// followed by another word. The first such word is the extra, and the
    /// Location is the line, counted from 1. Only the first match on a line is
    /// reported. Udebs are checked too.
    ChownWithDot,
    code = "LN1722",
    tag = "chown-with-dot",
    default_selected = false,
    experimental = false,
}

static CHOWN_WITH_DOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bchown\s+(?:-\S+\s+)*(\S+\.\S+)\b").expect("valid regex"));

impl BinaryPackageRule for ChownWithDot {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let mut hits = Vec::new();
        if let Some(files) = ctx.data_files() {
            hits.extend(scripts_with_dot(files));
        }
        if let Some(files) = ctx.control_files() {
            hits.extend(scripts_with_dot(files));
        }
        for (path, line, extra) in hits {
            ctx.diagnostic(
                Diagnostic::warn(extra)
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

/// `(path, line, owner.group)` for each script line that uses the dot form.
fn scripts_with_dot(files: &FileTree) -> Vec<(PathBuf, u32, String)> {
    let mut found = Vec::new();
    for entry in files.entries() {
        if !is_script_file(files, entry) {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        for (index, line) in bytes.split(|&byte| byte == b'\n').enumerate() {
            if line.starts_with(b"#") || line.is_empty() {
                continue;
            }
            let Some(captures) = CHOWN_WITH_DOT.captures(line) else {
                continue;
            };
            let ownership = String::from_utf8_lossy(&captures[1]).into_owned();
            found.push((
                entry.path().to_path_buf(),
                u32::try_from(index + 1).unwrap_or(u32::MAX),
                ownership,
            ));
        }
    }
    found
}

/// A file or hardlink whose content starts with a `#!` line.
fn is_script_file(files: &FileTree, entry: &Entry) -> bool {
    entry.is_file()
        && files
            .read(entry)
            .is_ok_and(|bytes| Hashbang::read(&bytes).is_some())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(content: &str) -> TestFile {
        TestFile::new(content)
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(vec![]),
            // The colon form is the modern one.
            case(vec![(
                "usr/bin/foo".to_string(),
                script("#!/bin/sh\nchown user:group /x\n"),
            )]),
            // Comments and empty lines are skipped.
            case(vec![(
                "usr/bin/foo".to_string(),
                script("#!/bin/sh\n# chown user.group /x\n\n"),
            )]),
            // Not a script: no hashbang.
            case(vec![(
                "usr/share/foo/data".to_string(),
                script("chown user.group\n"),
            )]),
            // Lincity-style `#!#!#!` data is not a script.
            case(vec![(
                "usr/share/foo/data".to_string(),
                script("#!#!#!\nchown user.group\n"),
            )]),
            // A dot only in a later word does not match after `chown`.
            case(vec![(
                "usr/bin/foo".to_string(),
                script("#!/bin/sh\nchown user /x/a.b\n"),
            )]),
            // Symlinks have no content to read.
            case(vec![("usr/bin/foo".to_string(), TestFile::symlink("bar"))]),
        ];
        let fail = vec![
            case(vec![(
                "usr/bin/foo".to_string(),
                script("#!/bin/sh\nchown someuser.theirgroup /x\n"),
            )]),
            // Options before the owner are skipped.
            case(vec![(
                "usr/bin/foo".to_string(),
                script("#!/bin/sh\nchown -R -v someuser.theirgroup /x\n"),
            )]),
            // Only the first match on a line is reported; the second line is too.
            case(vec![(
                "usr/bin/foo".to_string(),
                script("#!/bin/sh\nchown a.b c\nchown -R d.e f\n"),
            )]),
            // Maintainer scripts are control files and are checked too.
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
                .control_file("postinst", script("#!/bin/sh\nchown user.group /x\n")),
            // Udebs are checked too.
            case(vec![(
                "usr/bin/foo".to_string(),
                script("#!/bin/sh\nchown user.group /x\n"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(ChownWithDot, pass, fail).test_and_snapshot();
    }
}
