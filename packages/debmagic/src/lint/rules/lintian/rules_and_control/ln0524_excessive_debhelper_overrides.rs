use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::dh_scan::gated_scan;

pub struct ExcessiveDebhelperOverrides;

declare_rule! {
    /// `debian/rules` overrides twenty or more debhelper commands.
    ///
    /// Scans `debian/rules` the way lintian's `debhelper` Check does: `\`
    /// continuations are joined first, then comment lines are dropped. Targets
    /// of the form `override_`, `execute_before_`, or `execute_after_` plus
    /// `dh_<command>` and an optional `-arch` or `-indep` (any of several on a
    /// line, `%` wildcards excluded) count once per distinct command,
    /// whatever the timing word or focus. A target on a line that is itself a
    /// dh command, the `dh` sequencer, or a `DH_COMPAT` assignment is not
    /// seen. Twenty distinct commands report. Only reached after a dh command
    /// has been seen; a `Build-Driver` other than `debian-rules` is not
    /// scanned. Extra is the number of commands. There is no Location.
    /// Applies to the Source tree and Source package.
    ExcessiveDebhelperOverrides,
    code = "LN0524",
    tag = "excessive-debhelper-overrides",
    default_selected = true,
    experimental = false,
}

/// lintian 2.139.0 `$MANY_OVERRIDES`.
const MANY_OVERRIDES: usize = 20;

impl SourceRule for ExcessiveDebhelperOverrides {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((_, scan)) = gated_scan(ctx) else {
            return;
        };
        if scan.overrides.len() >= MANY_OVERRIDES {
            ctx.diagnostic(Diagnostic::warn(scan.overrides.len().to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    fn package(rules: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/rules", TestFile::new(rules)),
        ]
    }

    fn rules_with(count: usize) -> String {
        let mut rules = String::from("#!/usr/bin/make -f\n%:\n\tdh $@\n\n");
        for index in 0..count {
            rules.push_str(&format!("override_dh_cmd{index}:\n\ttrue\n"));
        }
        rules
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package(&rules_with(0)),
            package(&rules_with(19)),
            // The same command overridden at several timings counts once.
            package(&format!(
                "{}{}",
                rules_with(19),
                "execute_before_dh_cmd1:\nexecute_after_dh_cmd1-arch:\noverride_dh_cmd1-indep:\n"
            )),
            // Wildcards and comments do not count.
            package(&format!(
                "{}{}",
                rules_with(19),
                "override_dh_%:\n# override_dh_other:\n"
            )),
            // No dh command was seen.
            package(&rules_with(25).replace("\tdh $@\n", "\ttrue\n")),
        ];
        let fail = vec![
            package(&rules_with(20)),
            package(&rules_with(21)),
            package(&format!(
                "{}{}",
                rules_with(19),
                "execute_before_dh_other-arch override_dh_cmd3 :\n"
            )),
        ];
        SourceTreeTester::new(ExcessiveDebhelperOverrides, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(&rules_with(19)))];
        let fail = vec![SourcePackageCase::from(package(&rules_with(20)))];
        SourcePackageTester::new(ExcessiveDebhelperOverrides, pass, fail).test_and_snapshot();
    }
}
