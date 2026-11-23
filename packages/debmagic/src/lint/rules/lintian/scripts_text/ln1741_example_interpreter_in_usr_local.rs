use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::interpreter_hits;

pub struct ExampleInterpreterInUsrLocal;

declare_rule! {
    /// An example script's interpreter is under `/usr/local/`.
    ///
    /// The example variant of `interpreter-in-usr-local`. Only an installed
    /// regular file or hardlink under `usr/share/doc/<package>/examples/` is
    /// checked, and only when its interpreter's basename is not in lintian's
    /// interpreter table (a known interpreter is checked for its folder
    /// instead). Debian installs nothing under `/usr/local/`, so the example
    /// would need changes before it runs. Reported as a pedantic hint with
    /// Extra being the interpreter; the path is the Location. Udebs are checked
    /// too.
    ExampleInterpreterInUsrLocal,
    code = "LN1741",
    tag = "example-interpreter-in-usr-local",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for ExampleInterpreterInUsrLocal {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for hit in interpreter_hits::examples(files)
            .into_iter()
            .filter(|hit| hit.tag == "example-interpreter-in-usr-local")
        {
            ctx.diagnostic(
                Diagnostic::warn(hit.extra)
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path: hit.path,
                        line: hit.line,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn example(content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/doc/example/examples/run",
            TestFile::new(content).mode(0o755),
        )])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            example("#!/usr/bin/perl\n"),
            example("#!/bin/sh\n"),
            // A known interpreter under /usr/local is checked for its folder.
            example("#!/usr/local/bin/perl\n"),
            // Not an example: the non-example Tag.
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::new("#!/usr/local/bin/special-perl\n").mode(0o755),
            )]),
        ];
        let fail = vec![
            example("#!/usr/local/bin/special-perl\n"),
            // Not executable, but the example path makes it a script.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#!/usr/local/bin/special-perl\n"),
            )]),
            example("#!/usr/local/bin/special-perl\n").udeb(),
        ];
        BinaryPackageTester::new(ExampleInterpreterInUsrLocal, pass, fail).test_and_snapshot();
    }
}
