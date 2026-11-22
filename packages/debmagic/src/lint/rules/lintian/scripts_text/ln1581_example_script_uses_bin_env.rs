use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::interpreter_hits;

pub struct ExampleScriptUsesBinEnv;

declare_rule! {
    /// An example script names `/bin/env` as its interpreter.
    ///
    /// The example variant of `script-uses-bin-env`. Only an installed regular
    /// file or hardlink under `usr/share/doc/<package>/examples/` is checked.
    /// The interpreter is the first shebang word after a leading
    /// `/usr/bin/env` is removed, so `#!/bin/env perl` is `/bin/env` and
    /// `#!/usr/bin/env perl` is `perl`. Reported as an info hint with Extra
    /// `/bin/env`; the path is the Location. Udebs are checked too.
    ExampleScriptUsesBinEnv,
    code = "LN1581",
    tag = "example-script-uses-bin-env",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for ExampleScriptUsesBinEnv {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for hit in interpreter_hits::examples(files)
            .into_iter()
            .filter(|hit| hit.tag == "example-script-uses-bin-env")
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
                TestFile::new("#!/bin/env perl\n").mode(0o755),
            )]),
            // /usr/bin/env python3 is python3, not /bin/env.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#!/usr/bin/env python3\n").mode(0o755),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#!/bin/env perl\n").mode(0o755),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#! /bin/env sh\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/run",
                TestFile::new("#!/bin/env perl\n").mode(0o755),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(ExampleScriptUsesBinEnv, pass, fail).test_and_snapshot();
    }
}
