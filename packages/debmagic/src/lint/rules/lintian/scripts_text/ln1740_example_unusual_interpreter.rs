use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::interpreter_hits;

pub struct ExampleUnusualInterpreter;

declare_rule! {
    /// An example script's interpreter is neither in lintian's interpreter
    /// table nor shipped by the package.
    ///
    /// The example variant of `unusual-interpreter`. Only an installed regular
    /// file or hardlink under `usr/share/doc/<package>/examples/` is checked,
    /// and only when its interpreter is not in the table, is not `/usr/local/…`,
    /// `/bin/env`, `nodejs`, or `php…`, and no executable file of the package
    /// provides it (the path without its leading slash, or `bin/<name>` and
    /// `usr/bin/<name>` after `/usr/bin/env`). Reported as a pedantic hint with
    /// Extra being the interpreter; the path is the Location. Udebs are checked
    /// too.
    ExampleUnusualInterpreter,
    code = "LN1740",
    tag = "example-unusual-interpreter",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for ExampleUnusualInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for hit in interpreter_hits::examples(files)
            .into_iter()
            .filter(|hit| hit.tag == "example-unusual-interpreter")
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
            // Known interpreters.
            example("#!/bin/sh\n"),
            example("#!/usr/bin/python3\n"),
            // Interpreters that other Tags or no Tag cover.
            example("#!/usr/local/bin/zorg\n"),
            example("#!/bin/env zorg\n"),
            example("#!/usr/bin/env nodejs\n"),
            example("#!/usr/bin/php7.0\n"),
            // Not an example: the non-example Tag.
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::new("#!/usr/bin/zorg\n").mode(0o755),
            )]),
            // Shipped by the package and executable.
            BinaryPackageCase::from(vec![
                (
                    "usr/share/doc/example/examples/run",
                    TestFile::new("#!/usr/bin/env zorg\n").mode(0o755),
                ),
                ("bin/zorg", TestFile::new("").mode(0o755)),
            ]),
            BinaryPackageCase::from(vec![
                (
                    "usr/share/doc/example/examples/run",
                    TestFile::new("#!/usr/bin/zorg\n").mode(0o755),
                ),
                ("usr/bin/zorg", TestFile::new("").mode(0o755)),
            ]),
            // Not a script: an example Perl module that is not executable.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/lib.pm",
                TestFile::new("#!zorg\n"),
            )]),
        ];
        let fail = vec![
            example("#!/usr/bin/zorg\n"),
            example("#!/usr/bin/env zorg\n"),
            // The shipped interpreter is not executable.
            BinaryPackageCase::from(vec![
                (
                    "usr/share/doc/example/examples/run",
                    TestFile::new("#!/usr/bin/zorg\n").mode(0o755),
                ),
                ("usr/bin/zorg", TestFile::new("")),
            ]),
            example("#!/usr/bin/zorg\n").udeb(),
        ];
        BinaryPackageTester::new(ExampleUnusualInterpreter, pass, fail).test_and_snapshot();
    }
}
