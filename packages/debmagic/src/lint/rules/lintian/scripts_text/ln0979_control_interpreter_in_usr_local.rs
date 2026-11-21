use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::interpreter_hits::{control, control_facts};

pub struct ControlInterpreterInUsrLocal;

declare_rule! {
    /// A maintainer script (`config`, `preinst`, `postinst`, `prerm`, or
    /// `postrm`) names an interpreter under `/usr/local/`.
    ///
    /// Extra is the interpreter as written, such as `/usr/local/bin/bash`. The
    /// Location is the script name with no line number. ELF scripts are
    /// skipped. Udebs are checked.
    ControlInterpreterInUsrLocal,
    code = "LN0979",
    tag = "control-interpreter-in-usr-local",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ControlInterpreterInUsrLocal {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let facts = control_facts(ctx);
        let hits = match ctx.control_files() {
            Some(files) => control(files, &facts),
            None => return,
        };
        for hit in hits
            .into_iter()
            .filter(|hit| hit.tag == "control-interpreter-in-usr-local")
        {
            ctx.diagnostic(Diagnostic::error(hit.extra).with_location(Location {
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
            control_file("postinst", "#!/usr/bin/bash\n"),
            control_file("postinst", "#!/usr/bin/python3\n"),
            control_file("postinst", "#!/usr/bin/env python3\n"),
            control_file("README", "#!/usr/local/bin/bash\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "postinst",
                TestFile::new("#!/usr/local/bin/bash\n").file_type(
                    crate::lint::file_type::FileType::Elf {
                        has_static_symbol_table: false,
                        executable: true,
                    },
                ),
            ),
        ];
        let fail = vec![
            control_file("preinst", "#!/usr/local/bin/bash\nset -e\n"),
            control_file("postinst", "#!/usr/local/bin/perl\n"),
            control_file("config", "#!/usr/local/bin/python3\n"),
            control_file("prerm", "#!/usr/local/bin/python3 -u\n").udeb(),
        ];
        BinaryPackageTester::new(ControlInterpreterInUsrLocal, pass, fail).test_and_snapshot();
    }
}
