use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptShouldAlwaysStartService;

declare_rule! {
    /// An `etc/default` file can disable the service before the init system
    /// sees it.
    ///
    /// Each direct regular file or hardlink under `etc/default` is scanned.
    /// A line matches when, after spaces and `#` characters, it has an
    /// optional `[A-Z]_` prefix and then `ENABLED`, `DISABLED`, a run of
    /// capitals ending in `RUN`, or `START` / `NO_START`, immediately
    /// followed by `=`. A directory child does not stop the scan. `initscripts`
    /// and `sysvinit` are skipped. Extra is empty. The file path and line are
    /// the Location.
    InitDScriptShouldAlwaysStartService,
    code = "LN0290",
    tag = "init.d-script-should-always-start-service",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptShouldAlwaysStartService {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let Some(data) = ctx.data_files() else {
            return;
        };
        for hit in init_script::default_enable_lines(data, &package) {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: hit.path,
                line: Some(hit.line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn defaults(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(format!("etc/default/{name}"), TestFile::new(body))])
    }

    #[test]
    fn test() {
        let recipe = "\
ENABLED=\"false\"
ENABLED=false
DISABLED=\"true\"
DISABLED=true
RUN=\"true\"
RUN=true
PREFIXRUN=\"true\"
PREFIXRUN=true

# ENABLED=\"false\"
# ENABLED=false
# DISABLED=\"true\"
# DISABLED=true
# RUN=\"true\"
# RUN=true
# PREFIXRUN=\"true\"
# PREFIXRUN=true
";
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            defaults("lower", "enabled=1\n"),
            defaults("spaced", "ENABLED =1\n"),
            defaults("two-letters", "AB_ENABLED=1\n"),
            defaults("split-hash", "# # ENABLED=1\n"),
            BinaryPackageCase::from(vec![(
                "etc/default/sub/inner".to_string(),
                TestFile::new("ENABLED=1\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "etc/default/link".to_string(),
                TestFile::symlink("/etc/default/other"),
            )]),
            defaults("pkg", "ENABLED=1\n").package("initscripts"),
            defaults("pkg", "ENABLED=1\n").package("sysvinit"),
        ];
        let numbered = "ENABLED=1\nnope\n# DISABLED=1\n";
        let fail = vec![
            defaults("init.d-general", recipe),
            defaults("numbered", numbered),
            defaults("forms", "A_ENABLED=1\nNO_START=1\nMYRUN=1\nSTART=1\n"),
            BinaryPackageCase::from(vec![
                ("etc/default/a".to_string(), TestFile::new("ENABLED=1\n")),
                ("etc/default/sub".to_string(), TestFile::directory()),
                ("etc/default/z".to_string(), TestFile::new("RUN=1\n")),
            ]),
            BinaryPackageCase::from(vec![(
                "etc/default/hard".to_string(),
                TestFile::new("DISABLED=1\n").hardlink(),
            )]),
            defaults("udeb-default", "ENABLED=1\n").udeb(),
        ];
        BinaryPackageTester::new(InitDScriptShouldAlwaysStartService, pass, fail)
            .test_and_snapshot();
    }
}
