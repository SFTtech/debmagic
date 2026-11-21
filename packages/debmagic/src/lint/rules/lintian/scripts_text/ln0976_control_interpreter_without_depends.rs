use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::interpreter_hits::{control, control_facts};

pub struct ControlInterpreterWithoutDepends;

declare_rule! {
    /// A `prerm` or `postinst` maintainer script names a known interpreter
    /// whose package the package's strong dependencies (Pre-Depends and
    /// Depends) do not satisfy, such as `/usr/bin/python3` without
    /// `python3:any | python3-minimal:any`.
    ///
    /// Extra is the interpreter and `(does not satisfy …)` with the
    /// requirement in lintian's relation form. A `preinst` is
    /// control-interpreter-without-predepends, and `config` and `postrm` never
    /// reach this check. Skipped: the same cases as the unusual-interpreter
    /// Tag. Udebs are checked. The Location is the script name with no line
    /// number.
    ControlInterpreterWithoutDepends,
    code = "LN0976",
    tag = "control-interpreter-without-depends",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ControlInterpreterWithoutDepends {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let facts = control_facts(ctx);
        let hits = match ctx.control_files() {
            Some(files) => control(files, &facts),
            None => return,
        };
        for hit in hits
            .into_iter()
            .filter(|hit| hit.tag == "control-interpreter-without-depends")
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
            control_file("postinst", "#!/usr/bin/python3\n").field("Depends", "python3"),
            control_file("prerm", "#!/usr/bin/python3\n").field("Pre-Depends", "python3:any"),
            control_file("preinst", "#!/usr/bin/python3\n"),
            control_file("config", "#!/usr/bin/python3\n"),
            control_file("postrm", "#!/usr/bin/python3\n"),
            control_file("postinst", "#!/bin/sh\n"),
            control_file("postinst", "#!/usr/bin/unknown\n"),
        ];
        let fail = vec![
            control_file("postinst", "#!/usr/bin/python3\n"),
            control_file("prerm", "#!/bin/python3\n").field("Depends", "libc6"),
            control_file("postinst", "#!/usr/bin/python3\n")
                .field("Recommends", "python3")
                .udeb(),
        ];
        BinaryPackageTester::new(ControlInterpreterWithoutDepends, pass, fail).test_and_snapshot();
    }
}
