use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dh_scan::gated_scan;

pub struct DebianRulesUsesUnnecessaryDhArgument;

declare_rule! {
    /// `dh` is given an argument that the compat level makes unnecessary.
    ///
    /// Scans `debian/rules` the way lintian's `debhelper` Check does: `\`
    /// continuations are joined first, then comment lines are dropped. On
    /// an indented `dh` sequencer line, `--parallel` is unnecessary from
    /// compat level 10 and `--with` listing `systemd` is unnecessary from
    /// compat level 10 (the addon is matched after `-` becomes `_`). The last
    /// line carrying each argument counts, and `--parallel` reports before
    /// `systemd`. Only reached after a dh command has been seen; a
    /// `Build-Driver` other than `debian-rules` is not scanned. Extra is the
    /// level, `>= 10`, and `dh ... --parallel` or `dh ... --with=systemd`.
    /// Location is `debian/rules` and the line. Applies to the Source tree and
    /// Source package.
    DebianRulesUsesUnnecessaryDhArgument,
    code = "LN0525",
    tag = "debian-rules-uses-unnecessary-dh-argument",
    default_selected = true,
    experimental = false,
}

/// lintian 2.139.0 `$DH_PARALLEL_NOT_NEEDED`.
const DH_PARALLEL_NOT_NEEDED: u32 = 10;
/// lintian 2.139.0 `$INVOKES_SYSTEMD`.
const INVOKES_SYSTEMD: u32 = 10;

impl SourceRule for DebianRulesUsesUnnecessaryDhArgument {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((facts, scan)) = gated_scan(ctx) else {
            return;
        };
        let level = facts.level();
        let findings = [
            (
                scan.parallel_position,
                DH_PARALLEL_NOT_NEEDED,
                "dh ... --parallel",
            ),
            (
                scan.systemd_position,
                INVOKES_SYSTEMD,
                "dh ... --with=systemd",
            ),
        ];
        for (position, minimum, argument) in findings {
            if position == 0 || level < minimum {
                continue;
            }
            ctx.diagnostic(
                Diagnostic::warn(format!("{level} >= {minimum} {argument}")).with_location(
                    Location {
                        path: PathBuf::from("debian/rules"),
                        line: Some(position),
                        column: None,
                    },
                ),
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

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package("%:\n\tdh $@ --parallel\n", "9\n"),
            package("%:\n\tdh $@ --with systemd\n", "9\n"),
            package("%:\n\tdh $@ --with python3\n", "12\n"),
            package("%:\n\t# dh $@ --parallel --with systemd\n", "12\n"),
            // `--parallel` is only read on the `dh` sequencer line.
            package("%:\n\tdh $@\n\tdh_auto_build --parallel\n", "12\n"),
        ];
        let fail = vec![
            package("%:\n\tdh $@ --parallel\n", "10\n"),
            package("%:\n\tdh $@ --with systemd\n", "12\n"),
            package("%:\n\tdh $@ --with=python3,systemd --parallel\n", "13\n"),
            package(
                "%:\n\tdh $@ --parallel\n\n\tdh $@ --with systemd --parallel\n",
                "10\n",
            ),
        ];
        SourceTreeTester::new(DebianRulesUsesUnnecessaryDhArgument, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            "%:\n\tdh $@ --parallel\n",
            "9\n",
        ))];
        let fail = vec![SourcePackageCase::from(package(
            "%:\n\tdh $@ --parallel\n",
            "10\n",
        ))];
        SourcePackageTester::new(DebianRulesUsesUnnecessaryDhArgument, pass, fail)
            .test_and_snapshot();
    }
}
