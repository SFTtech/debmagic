use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dh_scan::gated_scan;

pub struct DebianRulesUsesDeprecatedSystemdOverride;

declare_rule! {
    /// `debian/rules` overrides `dh_systemd_enable` or `dh_systemd_start`.
    ///
    /// Scans `debian/rules` the way lintian's `debhelper` Check does: `\`
    /// continuations are joined first, then comment lines are dropped. A
    /// target (any of several on a line, `%` wildcards excluded) of the form
    /// `override_`, `execute_before_`, or `execute_after_` plus
    /// `dh_systemd_enable` / `dh_systemd_start` and an optional `-arch` or
    /// `-indep` reports when the declared compat level is 11 or newer. Only
    /// the last such target per command counts, whatever its timing word. A
    /// target on a line that is itself a dh command, the `dh` sequencer, or a
    /// `DH_COMPAT` assignment is not seen. Only reached after a dh command has
    /// been seen; a `Build-Driver` other than `debian-rules` is not scanned.
    /// Extra is `override_dh_systemd_enable` or `override_dh_systemd_start`
    /// plus the focus. Location is `debian/rules` and the line. Applies to the
    /// Source tree and Source package.
    DebianRulesUsesDeprecatedSystemdOverride,
    code = "LN0523",
    tag = "debian-rules-uses-deprecated-systemd-override",
    default_selected = true,
    experimental = false,
}

/// lintian 2.139.0 `$BETTER_SYSTEMD_INTEGRATION`.
const BETTER_SYSTEMD_INTEGRATION: u32 = 11;

impl SourceRule for DebianRulesUsesDeprecatedSystemdOverride {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((facts, scan)) = gated_scan(ctx) else {
            return;
        };
        if facts.level() < BETTER_SYSTEMD_INTEGRATION {
            return;
        }
        for suffix in ["enable", "start"] {
            let Some(found) = scan.overrides.get(&format!("dh_systemd_{suffix}")) else {
                continue;
            };
            ctx.diagnostic(
                Diagnostic::error(format!("override_dh_systemd_{suffix}{}", found.focus))
                    .with_location(Location {
                        path: PathBuf::from("debian/rules"),
                        line: Some(found.position),
                        column: None,
                    }),
            );
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

    fn package(rules: &str, compat: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/rules", TestFile::new(rules)),
            ("debian/compat", TestFile::new(compat)),
        ]
    }

    const DH: &str = "#!/usr/bin/make -f\n%:\n\tdh $@\n\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            // Compat 10 predates the better systemd integration.
            package(&format!("{DH}override_dh_systemd_enable:\n"), "10\n"),
            package(&format!("{DH}override_dh_installsystemd:\n"), "12\n"),
            package(&format!("{DH}override_dh_systemd_enable-%:\n"), "12\n"),
            package(&format!("{DH}# override_dh_systemd_start:\n"), "12\n"),
            package(&format!("{DH}override_dh_systemd_stop:\n"), "12\n"),
            // No dh command was seen.
            package("override_dh_systemd_enable:\n", "12\n"),
        ];
        let fail = vec![
            package(&format!("{DH}override_dh_systemd_enable:\n"), "11\n"),
            package(
                &format!(
                    "{DH}override_dh_systemd_enable-indep:\n\ttrue\noverride_dh_systemd_start-arch:\n"
                ),
                "12\n",
            ),
            // Only the last target of a command counts, with any timing word.
            package(
                &format!(
                    "{DH}override_dh_systemd_start:\nexecute_before_dh_systemd_start-indep:\n"
                ),
                "13\n",
            ),
            package(
                &format!("{DH}override_dh_clean override_dh_systemd_enable : foo\n"),
                "12\n",
            ),
        ];
        SourceTreeTester::new(DebianRulesUsesDeprecatedSystemdOverride, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            &format!("{DH}override_dh_systemd_enable:\n"),
            "10\n",
        ))];
        let fail = vec![SourcePackageCase::from(package(
            &format!("{DH}override_dh_systemd_start:\n"),
            "12\n",
        ))];
        SourcePackageTester::new(DebianRulesUsesDeprecatedSystemdOverride, pass, fail)
            .test_and_snapshot();
    }
}
