use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::file_type::FileType;
use crate::lint::rule::BinaryPackageRule;

use super::scan::{script_entries, shebang_token};

pub struct ForbiddenConfigInterpreter;

declare_rule! {
    /// `config` uses an interpreter other than `sh`, `bash`, or `perl`.
    ///
    /// The interpreter is the first shebang token, so `/usr/bin/env` stays
    /// when it is the command. `sh`, `bash`, and `perl` are a different Tag.
    /// An ELF `config` and a script with no interpreter are skipped. Extra is
    /// the interpreter path. The Location is `config`. Udebs are checked.
    ForbiddenConfigInterpreter,
    code = "LN0395",
    tag = "forbidden-config-interpreter",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ForbiddenConfigInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for interpreter in forbidden(files) {
            ctx.diagnostic(Diagnostic::error(interpreter).with_location(Location {
                path: PathBuf::from("config"),
                line: None,
                column: None,
            }));
        }
    }
}

fn forbidden(files: &FileTree) -> Vec<String> {
    script_entries(files)
        .into_iter()
        .filter(|(entry, _)| entry.path() == std::path::Path::new("config"))
        .filter(|(entry, _)| !is_elf(entry))
        .filter_map(|(_, bytes)| shebang_token(&bytes))
        .filter(|interpreter| !allowed(interpreter))
        .collect()
}

fn is_elf(entry: &Entry) -> bool {
    matches!(entry.file_type(), Some(FileType::Elf { .. }))
}

fn allowed(interpreter: &str) -> bool {
    matches!(
        interpreter.rsplit('/').next().unwrap_or(interpreter),
        "sh" | "bash" | "perl"
    )
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn config(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("config", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            config("#!/bin/sh\nset -e\n"),
            config("#!/usr/bin/bash\n"),
            config("#!/usr/bin/perl\n"),
            config("#! /bin/sh -e\n"),
            config("no shebang\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "config",
                TestFile::new("#!/usr/bin/python3\n").file_type(FileType::Elf {
                    has_static_symbol_table: false,
                    executable: true,
                }),
            ),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::new("#!/usr/bin/python3\n")),
        ];
        let fail = vec![
            config("#!/usr/bin/python3\n"),
            config("#!/usr/bin/python\n"),
            config("#!/usr/bin/env python3\n"),
            config("#!/usr/bin/python3 -u\n").udeb(),
        ];
        BinaryPackageTester::new(ForbiddenConfigInterpreter, pass, fail).test_and_snapshot();
    }
}
