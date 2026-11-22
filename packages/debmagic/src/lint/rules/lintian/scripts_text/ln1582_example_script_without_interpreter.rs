use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::interpreter_hits;

pub struct ExampleScriptWithoutInterpreter;

declare_rule! {
    /// An example script has a `#!` line with no interpreter.
    ///
    /// The example variant of `script-without-interpreter`. Only an installed
    /// regular file or hardlink under `usr/share/doc/<package>/examples/` whose
    /// hashbang is empty (a file that is only `#!`, possibly followed by
    /// whitespace) is reported. A file without `#!` is not a script. Reported
    /// as an info hint with no Extra; the path is the Location. Udebs are
    /// checked too.
    ExampleScriptWithoutInterpreter,
    code = "LN1582",
    tag = "example-script-without-interpreter",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for ExampleScriptWithoutInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for hit in interpreter_hits::examples(files)
            .into_iter()
            .filter(|hit| hit.tag == "example-script-without-interpreter")
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
            BinaryPackageCase::from(vec![("usr/bin/example", TestFile::new("#!\n").mode(0o755))]),
            // No #! line: not a script.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("echo hi\n").mode(0o755),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#!\n").mode(0o755),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#!   \necho hi\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#!\n"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(ExampleScriptWithoutInterpreter, pass, fail).test_and_snapshot();
    }
}
