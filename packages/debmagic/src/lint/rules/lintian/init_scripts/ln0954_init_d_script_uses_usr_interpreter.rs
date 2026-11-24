use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptUsesUsrInterpreter;

declare_rule! {
    /// An `etc/init.d` script's shebang interpreter is under `/usr/`.
    ///
    /// The interpreter is the first word of the hashbang, with a leading
    /// `/usr/bin/env` removed, so `#!/usr/bin/env perl` is not reported but
    /// `#!/usr/bin/env /usr/bin/perl` is. The extra is that interpreter and the
    /// Location is the script at line 1. The scripts checked are the ones the
    /// init-d check reads (see `init.d-script-contains-skeleton-template-content`
    /// for which scripts those are). `initscripts` and `sysvinit` are skipped.
    InitDScriptUsesUsrInterpreter,
    code = "LN0954",
    tag = "init.d-script-uses-usr-interpreter",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptUsesUsrInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let registered = ctx
            .control_files()
            .map(init_script::registered_names)
            .unwrap_or_default();
        let Some(data) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(PathBuf, String)> = init_script::init_d_scripts(data, &registered, &package)
            .into_iter()
            .filter_map(|script| {
                let interpreter = Hashbang::read(script.text.as_bytes())?.interpreter();
                interpreter
                    .starts_with("/usr/")
                    .then_some((script.path, interpreter))
            })
            .collect();
        for (path, interpreter) in hits {
            ctx.diagnostic(Diagnostic::warn(interpreter).with_location(Location {
                path,
                line: Some(1),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(name: &str, hashbang: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            format!("etc/init.d/{name}"),
            TestFile::new(format!("#!{hashbang}\n# Provides: {name}\n")),
        )])
    }

    #[test]
    fn binary() {
        let pass = vec![
            script("foo", "/bin/sh"),
            script("foo", "/usr/bin/env perl"),
            script("foo", "/lib/init/init-d-script"),
            script("foo", "/usr/bin/env /bin/sh"),
            script("skeleton", "/usr/bin/perl"),
            script("foo", "/usr/bin/perl").package("initscripts"),
        ];
        let fail = vec![
            script("foo", "/usr/bin/perl"),
            script("foo", "/usr/bin/env /usr/bin/perl"),
            script("foo", "/usr/bin/python3 -u").udeb(),
            script("rc", "/usr/bin/perl").control_file(
                "postinst",
                TestFile::new("#!/bin/sh\nupdate-rc.d rc defaults\n"),
            ),
        ];
        BinaryPackageTester::new(InitDScriptUsesUsrInterpreter, pass, fail).test_and_snapshot();
    }
}
