use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::interpreter_hits::{control, control_facts};

pub struct UnusualControlInterpreter;

declare_rule! {
    /// A maintainer script (`preinst`, `postinst`, `prerm`) names an interpreter
    /// that lintian knows by its unversioned name, such as `/usr/bin/python3`.
    ///
    /// Extra is the first shebang word, as written. The Location is the script
    /// name with no line number. Skipped: ELF scripts; `sh`, `bash`, and `perl`
    /// (the path Tags); `config` and `postrm` (their own forbidden-interpreter
    /// Tags); `/usr/local/` (its own Tag); and an interpreter that is unknown
    /// or known only as a versioned family. Udebs are checked.
    UnusualControlInterpreter,
    code = "LN0975",
    tag = "unusual-control-interpreter",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnusualControlInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let facts = control_facts(ctx);
        let hits = match ctx.control_files() {
            Some(files) => control(files, &facts),
            None => return,
        };
        for hit in hits
            .into_iter()
            .filter(|hit| hit.tag == "unusual-control-interpreter")
        {
            ctx.diagnostic(Diagnostic::warn(hit.extra).with_location(Location {
                path: hit.path,
                line: hit.line,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn control_file(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            control_file("postinst", "#!/bin/sh\nset -e\n"),
            control_file("postinst", "#!/usr/bin/perl\nprint 1;\n"),
            control_file("config", "#!/usr/bin/python3\nimport sys\n"),
            control_file("postrm", "#!/usr/bin/python3\nimport sys\n"),
            control_file("postinst", "#!/usr/bin/unknown\n"),
            control_file("postinst", "#!/usr/bin/tclsh8.6\n"),
            control_file("postinst", "#!/usr/local/bin/python3\n"),
            control_file("postinst", "no shebang\n"),
            control_file("README", "#!/usr/bin/python3\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "postinst",
                TestFile::new("#!/usr/bin/python3\n").file_type(
                    crate::lint::file_type::FileType::Elf {
                        has_static_symbol_table: false,
                        executable: true,
                    },
                ),
            ),
        ];
        let fail = vec![
            control_file("postinst", "#!/usr/bin/python3\nimport sys\n"),
            control_file("preinst", "#!/usr/bin/python3\nimport sys\n"),
            control_file("prerm", "#!/bin/python3\nimport sys\n"),
            control_file("postinst", "#!/usr/bin/python3 -u\n").udeb(),
        ];
        BinaryPackageTester::new(UnusualControlInterpreter, pass, fail).test_and_snapshot();
    }
}
