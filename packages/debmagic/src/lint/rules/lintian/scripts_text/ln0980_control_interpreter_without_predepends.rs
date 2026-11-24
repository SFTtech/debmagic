use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::interpreter_hits::{control, control_facts};

pub struct ControlInterpreterWithoutPredepends;

declare_rule! {
    /// A `preinst` names a known interpreter whose package the package's
    /// Pre-Depends does not satisfy, such as `/usr/bin/python3` without
    /// `python3:any | python3-minimal:any` in Pre-Depends. Depends does not
    /// count.
    ///
    /// Extra is the interpreter and `(does not satisfy …)` with the requirement
    /// in lintian's relation form. The Location is `preinst` with no line
    /// number. The same skips as the unusual-interpreter Tag apply. Udebs are
    /// checked.
    ControlInterpreterWithoutPredepends,
    code = "LN0980",
    tag = "control-interpreter-without-predepends",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ControlInterpreterWithoutPredepends {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let facts = control_facts(ctx);
        let hits = match ctx.control_files() {
            Some(files) => control(files, &facts),
            None => return,
        };
        for hit in hits
            .into_iter()
            .filter(|hit| hit.tag == "control-interpreter-without-predepends")
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
            control_file("preinst", "#!/usr/bin/python3\n").field("Pre-Depends", "python3"),
            control_file("preinst", "#!/bin/sh\n"),
            control_file("postinst", "#!/usr/bin/python3\n"),
            control_file("prerm", "#!/usr/bin/python3\n"),
            control_file("config", "#!/usr/bin/python3\n"),
            control_file("preinst", "#!/usr/bin/unknown\n"),
        ];
        let fail = vec![
            control_file("preinst", "#!/usr/bin/python3\n"),
            control_file("preinst", "#!/usr/bin/python3\n").field("Depends", "python3"),
            control_file("preinst", "#!/usr/bin/python3\n")
                .field("Pre-Depends", "libc6")
                .udeb(),
        ];
        BinaryPackageTester::new(ControlInterpreterWithoutPredepends, pass, fail)
            .test_and_snapshot();
    }
}
