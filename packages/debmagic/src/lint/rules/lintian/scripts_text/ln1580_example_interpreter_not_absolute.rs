use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::interpreter_hits;

pub struct ExampleInterpreterNotAbsolute;

declare_rule! {
    /// An example script's interpreter is a relative path, such as `#!perl`.
    ///
    /// The example variant of `interpreter-not-absolute`. Only an installed
    /// regular file or hardlink under `usr/share/doc/<package>/examples/` is
    /// checked, whether or not it is executable. The interpreter is the first
    /// word after a leading `/usr/bin/env` is removed, and `/usr/bin/env` makes
    /// the script absolute. Reported as an info hint with the interpreter as
    /// Extra; the path is the Location. A `.in` file with a template token as
    /// its interpreter is skipped. Udebs are checked too.
    ExampleInterpreterNotAbsolute,
    code = "LN1580",
    tag = "example-interpreter-not-absolute",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for ExampleInterpreterNotAbsolute {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for hit in interpreter_hits::examples(files)
            .into_iter()
            .filter(|hit| hit.tag == "example-interpreter-not-absolute")
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
                TestFile::new("#!perl\n").mode(0o755),
            )]),
            // Calling through /usr/bin/env is absolute.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#!/usr/bin/env perl\n"),
            )]),
            // A .in template token is skipped.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run.in",
                TestFile::new("#!@PERL@\n"),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#!perl\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#!perl\n").mode(0o755),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#!env perl\n"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(ExampleInterpreterNotAbsolute, pass, fail).test_and_snapshot();
    }
}
