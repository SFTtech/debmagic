use std::os::unix::ffi::OsStrExt;
use std::path::{Component, PathBuf};
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::interpreter::{Hashbang, basename};
use crate::lint::rule::BinaryPackageRule;

pub struct ScriptUsesPerl4LibsWithoutDep;

declare_rule! {
    /// An installed Perl script loads an obsolete perl 4 library with `do` or
    /// `require` and the package does not satisfy the prerequisite.
    ///
    /// Each Data files Entry that is a regular file, a hardlink, or a symlink
    /// resolving to one inside the tree (lintian's `is_open_ok`) and whose
    /// Hashbang interpreter basename is `perl`. Entries under `usr/share/doc/`
    /// and `usr/src/` are skipped, except `usr/share/doc/<pkg>/examples/`.
    /// Each line that matches `do` or `require` followed by a quoted perl 4
    /// module name ending in `.pl` (abbrev.pl, getopt.pl, ...) is reported
    /// once, with the module name and line number. The prerequisite is
    /// `libperl4-corelibs-perl:any | perl:any (<< 5.12.3-7)`, checked against
    /// Pre-Depends and Depends of the Installation control (lintian's `strong`
    /// relation); a satisfied package reports nothing. Location is the
    /// installed path and line. Udebs are checked too.
    ScriptUsesPerl4LibsWithoutDep,
    code = "LN1111",
    tag = "script-uses-perl4-libs-without-dep",
    default_selected = true,
    experimental = false,
}

const PERL4_PREREQUISITES: &str = "libperl4-corelibs-perl:any | perl:any (<< 5.12.3-7)";

static EXAMPLES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/doc/[^/]+/examples/").expect("valid regex"));
static PERL4_MODULE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?:do|require)\s+['"](abbrev|assert|bigfloat|bigint|bigrat|cacheout|complete|ctime|dotsh|exceptions|fastcwd|find|finddepth|flush|getcwd|getopt|getopts|hostname|importenv|look|newgetopt|open2|open3|pwd|shellwords|stat|syslog|tainted|termcap|timelocal|validate)\.pl['"]"#,
    )
    .expect("valid regex")
});

impl BinaryPackageRule for ScriptUsesPerl4LibsWithoutDep {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let satisfied = ctx
            .installation_control()
            .fields()
            .relations_and(&["Pre-Depends", "Depends"])
            .satisfies(PERL4_PREREQUISITES);
        if satisfied {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits = perl4_uses(files);
        for (path, line, module) in hits {
            ctx.diagnostic(
                Diagnostic::warn(format!("(does not satisfy {PERL4_PREREQUISITES}) {module}"))
                    .with_location(Location {
                        path,
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

/// Perl scripts in file-tree order, each perl 4 module line as
/// `(installed path, line number, module name)`.
fn perl4_uses(files: &FileTree) -> Vec<(PathBuf, u32, String)> {
    let mut hits = Vec::new();
    for entry in files.entries() {
        let name = entry.path().as_os_str().as_bytes();
        let skipped_doc = (name.starts_with(b"usr/share/doc/") || name.starts_with(b"usr/src/"))
            && !EXAMPLES.is_match(name);
        if skipped_doc {
            continue;
        }
        let Some(target) = open_target(files, entry) else {
            continue;
        };
        let Ok(bytes) = files.read(target) else {
            continue;
        };
        let Some(hashbang) = Hashbang::read(&bytes) else {
            continue;
        };
        if basename(&hashbang.interpreter()) != "perl" {
            continue;
        }
        for (index, line) in bytes.split(|&byte| byte == b'\n').enumerate() {
            let Some(captures) = PERL4_MODULE.captures(line) else {
                continue;
            };
            let module = format!("{}.pl", String::from_utf8_lossy(&captures[1]));
            let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
            hits.push((entry.path().to_path_buf(), line_number, module));
        }
    }
    hits
}

/// The regular file or hardlink `entry` names, following relative symlinks
/// that stay inside the tree (lintian's `is_open_ok`). `None` for directories,
/// special files, dangling, absolute, or escaping links, and loops.
fn open_target<'t>(files: &'t FileTree, entry: &'t Entry) -> Option<&'t Entry> {
    let mut current = entry;
    for _ in 0..8 {
        match current.kind() {
            EntryKind::RegularFile | EntryKind::Hardlink => return Some(current),
            EntryKind::Symlink => {
                let mut resolved: PathBuf = current.path().parent()?.to_path_buf();
                for component in current.link_target()?.components() {
                    match component {
                        Component::Normal(part) => resolved.push(part),
                        Component::CurDir => {}
                        Component::ParentDir => {
                            if !resolved.pop() {
                                return None;
                            }
                        }
                        Component::RootDir | Component::Prefix(_) => return None,
                    }
                }
                current = files.get(&resolved)?;
            }
            EntryKind::Directory
            | EntryKind::Fifo
            | EntryKind::CharDevice
            | EntryKind::BlockDevice => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn perl(body: &str) -> TestFile {
        TestFile::new(format!("#!/usr/bin/perl\n{body}"))
    }

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // The package satisfies the prerequisite, through either field.
            data("usr/bin/tool", perl("do 'newgetopt.pl';\n"))
                .field("Depends", "perl (<< 5.12.3-7)"),
            data("usr/bin/tool", perl("do 'newgetopt.pl';\n"))
                .field("Pre-Depends", "libperl4-corelibs-perl"),
            // Not a perl interpreter.
            data(
                "usr/bin/tool",
                TestFile::new("#!/bin/sh\ndo 'newgetopt.pl';\n"),
            ),
            data("usr/bin/tool", TestFile::new("do 'newgetopt.pl';\n")),
            // Lincity data magic is not a script.
            data(
                "usr/share/games/lincity",
                TestFile::new("#!#!#!\ndo 'newgetopt.pl';\n"),
            ),
            // Documentation and source are skipped.
            data("usr/share/doc/foo/script", perl("do 'newgetopt.pl';\n")),
            data("usr/src/foo/script", perl("do 'newgetopt.pl';\n")),
            // Not a perl 4 library, or not a `.pl` file.
            data("usr/bin/tool", perl("do 'Foo::Bar.pm';\n")),
            data("usr/bin/tool", perl("require 'newgetopt.pm';\n")),
            data("usr/bin/tool", perl("use newgetopt;\n")),
            data("usr/bin/tool", perl("do 'myfind.pl';\n")),
            // Dangling and escaping symlinks are not opened.
            data("usr/bin/tool", TestFile::symlink("missing")),
            data("usr/bin/tool", TestFile::symlink("/usr/share/foo")),
            // A directory is not a script.
            data("usr/bin/tool", TestFile::directory()),
        ];
        let fail = vec![
            data("usr/bin/tool", perl("do 'newgetopt.pl';\n")),
            // Only lines that load a module count, with their line numbers.
            data(
                "usr/bin/tool",
                perl("use strict;\nrequire \"getopt.pl\";\n\ndo 'find.pl';\n"),
            ),
            // The package lacks the prerequisite (a plain perl does not satisfy it).
            data("usr/bin/tool", perl("do 'newgetopt.pl';\n")).field("Depends", "perl"),
            // Recommends does not count (lintian's `strong` relation).
            data("usr/bin/tool", perl("do 'newgetopt.pl';\n"))
                .field("Recommends", "libperl4-corelibs-perl"),
            // Documentation examples are checked.
            data(
                "usr/share/doc/foo/examples/script",
                perl("do 'newgetopt.pl';\n"),
            ),
            // Hardlinks and symlinks into the tree are opened.
            data("usr/bin/tool", perl("do 'newgetopt.pl';\n").hardlink()),
            BinaryPackageCase::from(vec![
                ("usr/bin/tool", perl("do 'newgetopt.pl';\n")),
                ("usr/bin/link", TestFile::symlink("tool")),
            ]),
            // Udebs are checked too.
            data("usr/bin/tool", perl("do 'newgetopt.pl';\n")).udeb(),
            // A non-UTF-8 name is reported with its bytes.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/bin/caf\xe9".to_vec()),
                perl("do 'newgetopt.pl';\n"),
            )]),
        ];
        BinaryPackageTester::new(ScriptUsesPerl4LibsWithoutDep, pass, fail).test_and_snapshot();
    }
}
