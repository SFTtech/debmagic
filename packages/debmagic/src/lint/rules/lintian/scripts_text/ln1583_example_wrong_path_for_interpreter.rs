use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::interpreter_hits;

pub struct ExampleWrongPathForInterpreter;

declare_rule! {
    /// An example script names a known interpreter at a path other than its
    /// table folder.
    ///
    /// The example variant of `wrong-path-for-interpreter`. Only an installed
    /// regular file or hardlink under `usr/share/doc/<package>/examples/` whose
    /// interpreter is in lintian's table is checked, and not one that calls
    /// `/usr/bin/env`. Extra is `<interpreter> != <folder>/<basename>`. For a
    /// table folder under `/usr/bin/`, `/usr/sbin/`, or `/usr/lib/` (not perl),
    /// the path without `/usr` is also accepted, and the Extra then reads
    /// `<interpreter> != <non-usr path> OR <expected>`. Reported as an info
    /// hint; the path is the Location. Udebs are checked too.
    ExampleWrongPathForInterpreter,
    code = "LN1583",
    tag = "example-wrong-path-for-interpreter",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for ExampleWrongPathForInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for hit in interpreter_hits::examples(files)
            .into_iter()
            .filter(|hit| hit.tag == "example-wrong-path-for-interpreter")
        {
            ctx.diagnostic(
                Diagnostic::warn(hit.extra)
                    .with_severity(Severity::Info)
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

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Not an example: the non-example Tag.
            BinaryPackageCase::from(vec![(
                "usr/bin/example",
                TestFile::new("#!/usr/local/bin/python3\n").mode(0o755),
            )]),
            // The expected folder is accepted.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run.py",
                TestFile::new("#!/usr/bin/python3\n").mode(0o755),
            )]),
            // The usr-merged path is accepted for sh.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#!/bin/sh\n").mode(0o755),
            )]),
            // Calling through /usr/bin/env is not checked here.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run.py",
                TestFile::new("#!/usr/bin/env python3\n").mode(0o755),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run.py",
                TestFile::new("#!/usr/local/bin/python3\n").mode(0o755),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#!/opt/bin/bash\n").mode(0o755),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run.py",
                TestFile::new("#!/usr/local/bin/python3\n").mode(0o755),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(ExampleWrongPathForInterpreter, pass, fail).test_and_snapshot();
    }
}
