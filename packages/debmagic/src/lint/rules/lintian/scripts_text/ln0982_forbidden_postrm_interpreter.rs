use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::file_type::FileType;
use crate::lint::rule::BinaryPackageRule;

use super::scan::{script_entries, shebang_token};

pub struct ForbiddenPostrmInterpreter;

declare_rule! {
    /// A `postrm` runs an interpreter other than `sh`, `bash`, or `perl`.
    ///
    /// `purge` can only rely on essential packages. The interpreter is the
    /// first word of the shebang, so `/usr/bin/env` is reported as itself.
    /// A `/usr/local/` interpreter is `control-interpreter-in-usr-local`,
    /// a different Tag. An ELF `postrm` is skipped. Extra is the
    /// interpreter; the Location is `postrm`. Udebs are checked.
    ForbiddenPostrmInterpreter,
    code = "LN0982",
    tag = "forbidden-postrm-interpreter",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ForbiddenPostrmInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for interpreter in forbidden(files) {
            ctx.diagnostic(Diagnostic::error(interpreter).with_location(Location {
                path: PathBuf::from("postrm"),
                line: None,
                column: None,
            }));
        }
    }
}

fn forbidden(files: &FileTree) -> Vec<String> {
    script_entries(files)
        .into_iter()
        .filter(|(entry, _)| entry.path() == Path::new("postrm"))
        .filter(|(entry, _)| !is_elf(entry))
        .filter_map(|(_, bytes)| shebang_token(&bytes))
        .filter(|interpreter| !interpreter.starts_with("/usr/local/") && !allowed(interpreter))
        .collect()
}

fn allowed(interpreter: &str) -> bool {
    matches!(
        interpreter.rsplit('/').next().unwrap_or(interpreter),
        "sh" | "bash" | "perl"
    )
}

fn is_elf(entry: &Entry) -> bool {
    matches!(entry.file_type(), Some(FileType::Elf { .. }))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn postrm(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("postrm", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            postrm("#!/bin/sh\nset -e\n"),
            postrm("#!/bin/bash\n"),
            postrm("#!/usr/bin/perl\n"),
            postrm("no shebang\n"),
            // Under /usr/local/ is another Tag.
            postrm("#!/usr/local/bin/python3\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "postrm",
                TestFile::new("#!/usr/bin/python3\n").file_type(FileType::Elf {
                    has_static_symbol_table: false,
                    executable: true,
                }),
            ),
            // Other maintainer scripts are other Tags.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::new("#!/usr/bin/python3\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("config", TestFile::new("#!/usr/bin/python3\n")),
        ];
        let fail = vec![
            postrm("#!/usr/bin/python3\n"),
            postrm("#!/usr/bin/python3 -u\n").udeb(),
            postrm("#!/usr/bin/env python3\n"),
            postrm("#!/opt/bin/tool\n"),
        ];
        BinaryPackageTester::new(ForbiddenPostrmInterpreter, pass, fail).test_and_snapshot();
    }
}
