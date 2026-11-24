use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::interpreter_hits::{installed, installed_prerequisites};

pub struct PhpScriptWithUnusualInterpreter;

declare_rule! {
    /// An installed script's interpreter starts with `php` but is not a name
    /// lintian knows, such as `php7.0`. `php` itself is in lintian's table and
    /// is not this Tag.
    ///
    /// Extra is the interpreter, with a leading `/usr/bin/env` removed. The
    /// Location is the path with no line number. Only files outside
    /// `usr/share/doc/` (examples are a different Tag), `usr/src/`, and the
    /// other non-script cases of the scripts walk are read. A non-executable
    /// file is skipped unless it is in a `bin/` directory or has an absolute
    /// interpreter.
    PhpScriptWithUnusualInterpreter,
    code = "LN0978",
    tag = "php-script-with-unusual-interpreter",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PhpScriptWithUnusualInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let prerequisites = installed_prerequisites(ctx);
        let hits = match ctx.data_files() {
            Some(files) => installed(files, &prerequisites),
            None => return,
        };
        for hit in hits
            .into_iter()
            .filter(|hit| hit.tag == "php-script-with-unusual-interpreter")
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
            script("usr/bin/tool", "#!/usr/bin/php\n", 0o755),
            script("usr/bin/tool", "#!/usr/bin/env php\n", 0o755),
            script("usr/share/scripts/tool", "#!php7.0\n", 0o644),
            script("usr/bin/tool", "#!/usr/bin/python3\n", 0o755),
            script(
                "usr/share/doc/pkg/examples/tool",
                "#!/usr/bin/php7.0\n",
                0o755,
            ),
            script("usr/share/doc/pkg/tool", "#!/usr/bin/php7.0\n", 0o755),
            script("usr/src/pkg/tool", "#!/usr/bin/php7.0\n", 0o755),
        ];
        let fail = vec![
            script("usr/share/scripts/tool", "#!/usr/bin/env php7.0\n", 0o755),
            script("usr/share/scripts/tool", "#!/usr/bin/php7.0\n", 0o755),
            script("usr/bin/tool", "#!/usr/bin/php7.0 -q\n", 0o755).udeb(),
        ];
        BinaryPackageTester::new(PhpScriptWithUnusualInterpreter, pass, fail).test_and_snapshot();
    }
}
