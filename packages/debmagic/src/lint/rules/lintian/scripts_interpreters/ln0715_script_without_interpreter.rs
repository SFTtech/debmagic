use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::file_type::FileType;
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::BinaryPackageRule;

pub struct ScriptWithoutInterpreter;

declare_rule! {
    /// A script starts with `#!` but names no interpreter.
    ///
    /// Installed regular Data files whose first line is `#!` followed by
    /// nothing but whitespace. Such a file is reported only when it is
    /// executable or lies under a `bin/` or `etc/init.d/` directory.
    /// Paths under `usr/share/doc/`, `usr/src/`, and `usr/share/cargo/registry/`
    /// are skipped; examples are a different Tag. Maintainer scripts (Control
    /// files named `config…` or ending in `preinst`, `postinst`, `prerm`, or
    /// `postrm`) are checked unless they are ELF; one with no `#!` line or an
    /// empty hashbang is reported. Extra is empty. The Location is the path,
    /// or the Control file name. Udebs are checked too.
    ScriptWithoutInterpreter,
    code = "LN0715",
    tag = "script-without-interpreter",
    default_selected = true,
    experimental = false,
}

static MAINTAINER_SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^config|(?:pre|post)(?:inst|rm)$").expect("valid regex"));

impl BinaryPackageRule for ScriptWithoutInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let mut paths = Vec::new();
        if let Some(files) = ctx.data_files() {
            paths.extend(installed(files));
        }
        if let Some(files) = ctx.control_files() {
            paths.extend(maintainer_scripts(files));
        }
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Installed scripts with an empty interpreter, outside the skipped trees.
fn installed(files: &FileTree) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .into_iter()
        .filter_map(|entry| {
            let name = entry.path().to_str()?;
            if skipped(name) {
                return None;
            }
            // lintian ignores a non-executable file unless it is under bin/ or
            // etc/init.d/; the Entry is a regular file already.
            let in_bin = name.contains("bin/") || name.contains("etc/init.d/");
            if !in_bin && !entry.is_executable_file() {
                return None;
            }
            let bytes = files.read(entry).ok()?;
            let hashbang = Hashbang::read(&bytes)?;
            hashbang
                .interpreter()
                .is_empty()
                .then(|| entry.path().to_path_buf())
        })
        .collect()
}

/// Maintainer scripts with no `#!` line or an empty one, outside ELF.
fn maintainer_scripts(files: &FileTree) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .path(&MAINTAINER_SCRIPT)
        .into_iter()
        .filter(|entry| !is_elf(entry))
        .filter_map(|entry| {
            let bytes = files.read(entry).ok()?;
            let empty =
                Hashbang::read(&bytes).is_none_or(|hashbang| hashbang.interpreter().is_empty());
            empty.then(|| entry.path().to_path_buf())
        })
        .collect()
}

fn skipped(name: &str) -> bool {
    name.starts_with("usr/share/doc/")
        || name.starts_with("usr/src/")
        || name.starts_with("usr/share/cargo/registry/")
}

fn is_elf(entry: &Entry) -> bool {
    matches!(entry.file_type(), Some(FileType::Elf { .. }))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content).mode(0o755))])
    }

    fn maintainer(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("usr/bin/tool", "#!/bin/sh\n"),
            script("usr/bin/tool", "#!/usr/bin/env\n"),
            script("usr/bin/tool", "#!#!#!\n"),
            script("usr/bin/tool", "no shebang\n"),
            // Not executable and not under bin/ or etc/init.d/.
            BinaryPackageCase::from(vec![("usr/share/foo/tool", TestFile::new("#!\n"))]),
            // Not a regular file: a hardlink to a script with an interpreter.
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::new("#!\n").mode(0o755).hardlink(),
            )]),
            // Skipped trees.
            script("usr/share/doc/pkg/tool", "#!\n"),
            script("usr/share/doc/pkg/examples/tool", "#!\n"),
            script("usr/src/pkg/tool", "#!\n"),
            script("usr/share/cargo/registry/pkg/tool", "#!\n"),
            // ELF maintainer scripts are another Tag.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "postinst",
                TestFile::new("#!\n").file_type(FileType::Elf {
                    has_static_symbol_table: false,
                    executable: true,
                }),
            ),
            maintainer("postinst", "#!/bin/sh\n"),
            maintainer("preinst", "#! /bin/sh -e\n"),
            maintainer("config", "#!/usr/bin/perl\n"),
            // Not a maintainer script name.
            maintainer("templates", "no shebang\n"),
            maintainer("README", "#!\n"),
        ];
        let fail = vec![
            script("usr/bin/no-interpreter", "#!\n"),
            script("usr/bin/no-interpreter", "#!   \n"),
            script("usr/bin/no-interpreter", "#!\n#!/bin/sh\n"),
            // Under bin/ or etc/init.d/, executable or not.
            BinaryPackageCase::from(vec![("usr/lib/bin/tool", TestFile::new("#!\n"))]),
            BinaryPackageCase::from(vec![("etc/init.d/tool", TestFile::new("#!\n"))]),
            // Not skipped: the family of doc-base is not doc/.
            script("usr/share/doc-base/tool", "#!\n"),
            BinaryPackageCase::from(vec![(
                "usr/lib/cgi-bin/tool",
                TestFile::new("#!\n").mode(0o755),
            )])
            .udeb(),
            maintainer("postrm", "no shebang\n"),
            maintainer("postinst", ""),
            maintainer("config", "#!\n"),
            maintainer("preinst", "#!#!#!\n"),
            maintainer("prerm", "#!  \n").udeb(),
        ];
        BinaryPackageTester::new(ScriptWithoutInterpreter, pass, fail).test_and_snapshot();
    }
}
