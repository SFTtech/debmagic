use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::interpreter_hits;

pub struct ExampleIncorrectPathForInterpreter;

declare_rule! {
    /// An example script starts with `#!/usr/bin/env perl`.
    ///
    /// The example variant of `incorrect-path-for-interpreter`: Policy 10.4
    /// says Perl scripts use `/usr/bin/perl` directly. Only an installed
    /// regular file or hardlink under `usr/share/doc/<package>/examples/`
    /// whose interpreter is `perl` through `/usr/bin/env` is reported. Reported
    /// as an info hint with Extra `/usr/bin/env perl != /usr/bin/perl`; the
    /// path is the Location, and the executable bit is not checked. Udebs are
    /// checked too.
    ExampleIncorrectPathForInterpreter,
    code = "LN1579",
    tag = "example-incorrect-path-for-interpreter",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for ExampleIncorrectPathForInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for hit in interpreter_hits::examples(files)
            .into_iter()
            .filter(|hit| hit.tag == "example-incorrect-path-for-interpreter")
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
                TestFile::new("#!/usr/bin/env perl\n").mode(0o755),
            )]),
            // Absolute perl is the policy form.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run.pl",
                TestFile::new("#!/usr/bin/perl\n").mode(0o755),
            )]),
            // Another interpreter through env is not this Tag.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run.py",
                TestFile::new("#!/usr/bin/env python3\n").mode(0o755),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run.pl",
                TestFile::new("#!/usr/bin/env perl\n").mode(0o755),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run.pl",
                TestFile::new("#!/usr/bin/env perl -w\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run.pl",
                TestFile::new("#!/usr/bin/env perl\n").mode(0o755),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(ExampleIncorrectPathForInterpreter, pass, fail)
            .test_and_snapshot();
    }
}
