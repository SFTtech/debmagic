use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::file_type::FileType;
use crate::lint::interpreter::{basename, unversioned};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct UnknownControlInterpreter;

declare_rule! {
    /// A maintainer script starts with an interpreter this table does not know.
    ///
    /// The interpreter is the first shebang word, options dropped, with a
    /// leading `/usr/bin/env` kept, so `#!/usr/bin/env python3` is
    /// `/usr/bin/env`. It is unknown when its basename is not in lintian's
    /// `scripts/interpreters` table. Scripts whose interpreter is under
    /// `/usr/local/`, `sh`, `bash`, or `perl` are other Tags, and so are
    /// `config` and `postrm`, whose interpreter Tags come first. Scripts
    /// without a shebang and ELF maintainer scripts are skipped. Extra is the
    /// interpreter; the Location is the script with no line number.
    UnknownControlInterpreter,
    code = "LN1189",
    tag = "unknown-control-interpreter",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnknownControlInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for (path, interpreter) in unknown(files) {
            ctx.diagnostic(Diagnostic::error(interpreter).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Maintainer scripts whose interpreter is not in the table, with the interpreter.
fn unknown(files: &FileTree) -> Vec<(PathBuf, String)> {
    scan::script_entries(files)
        .into_iter()
        .filter_map(|(entry, bytes)| {
            if matches!(entry.file_type(), Some(FileType::Elf { .. })) {
                return None;
            }
            let interpreter = scan::shebang_token(&bytes)?;
            if interpreter.starts_with("/usr/local/") {
                return None;
            }
            let base = basename(&interpreter);
            if matches!(base, "sh" | "bash" | "perl") {
                return None;
            }
            let name = entry.path().to_str()?;
            if name == "config" || name == "postrm" {
                return None;
            }
            if unversioned(base).is_some() {
                return None;
            }
            Some((entry.path().to_path_buf(), interpreter))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn control(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Known interpreters, with or without options.
            control("postinst", "#!/bin/sh\nset -e\n"),
            control("postinst", "#!/bin/bash\n"),
            control("postinst", "#!/usr/bin/perl\n"),
            control("postinst", "#!/usr/bin/python3 -u\n"),
            control("preinst", "#!/usr/bin/python3\n"),
            // Other Tags: /usr/local, and the config and postrm interpreter Tags.
            control("postinst", "#!/usr/local/bin/foo\n"),
            control("config", "#!/usr/bin/foo\n"),
            control("postrm", "#!/usr/bin/foo\n"),
            // No shebang, not a maintainer script, or an empty interpreter.
            control("postinst", "echo hi\n"),
            control("postinst", "#!\n"),
            control("README", "#!/usr/bin/foo\n"),
        ];
        let fail = vec![
            control("postinst", "#!/usr/bin/unknown\n"),
            control("prerm", "#!/usr/bin/unknown -x\n"),
            control("preinst", "#!/usr/bin/env python3\n"),
            control("postinst", "#!/usr/bin/unknown\n").udeb(),
        ];
        BinaryPackageTester::new(UnknownControlInterpreter, pass, fail).test_and_snapshot();
    }
}
