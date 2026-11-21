use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::interpreter_hits::{installed, installed_prerequisites};

pub struct ExampleScriptUsesDeprecatedNodejsLocation;

declare_rule! {
    /// An example script under `usr/share/doc/<package>/examples/` names
    /// `nodejs` as its interpreter, the deprecated location of Node.js.
    ///
    /// The interpreter is the first shebang word after a leading
    /// `/usr/bin/env` is removed, so `#!/usr/bin/env nodejs` is `nodejs`.
    /// Extra is `nodejs`. The Location is the path with no line number. Other
    /// installed scripts are script-uses-deprecated-nodejs-location.
    ExampleScriptUsesDeprecatedNodejsLocation,
    code = "LN0981",
    tag = "example-script-uses-deprecated-nodejs-location",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ExampleScriptUsesDeprecatedNodejsLocation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let prerequisites = installed_prerequisites(ctx);
        let hits = match ctx.data_files() {
            Some(files) => installed(files, &prerequisites),
            None => return,
        };
        for hit in hits
            .into_iter()
            .filter(|hit| hit.tag == "example-script-uses-deprecated-nodejs-location")
        {
            ctx.diagnostic(Diagnostic::warn(hit.extra).with_location(Location {
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

    fn script(path: &str, content: &str, mode: u32) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content).mode(mode))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("usr/bin/tool", "#!/usr/bin/env nodejs\n", 0o755),
            script(
                "usr/share/doc/pkg/examples/tool",
                "#!/usr/bin/env node\n",
                0o644,
            ),
            script(
                "usr/share/doc/pkg/examples/tool",
                "#!/usr/bin/env python3\n",
                0o755,
            ),
            script(
                "usr/share/doc/pkg/examples/tool",
                "#!/usr/bin/nodejs\n",
                0o755,
            ),
            script("usr/share/doc/pkg/tool", "#!/usr/bin/env nodejs\n", 0o755),
            script("usr/src/pkg/tool", "#!/usr/bin/env nodejs\n", 0o755),
        ];
        let fail = vec![
            script(
                "usr/share/doc/pkg/examples/tool",
                "#!/usr/bin/env nodejs\nconsole.log(1);\n",
                0o644,
            ),
            script(
                "usr/share/doc/pkg/examples/tool",
                "#!/usr/bin/env nodejs\n",
                0o755,
            )
            .udeb(),
        ];
        BinaryPackageTester::new(ExampleScriptUsesDeprecatedNodejsLocation, pass, fail)
            .test_and_snapshot();
    }
}
