use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dh_scan::gated_scan;

pub struct PackageUsesDebhelperButLacksBuildDepends;

declare_rule! {
    /// `debian/rules` uses debhelper without a build dependency on it.
    ///
    /// Scans `debian/rules` the way lintian's `debhelper` Check does: `\`
    /// continuations are joined first, then comment lines are dropped. A
    /// `dh_*` command (other than `dh_autoreconf`), the `dh` sequencer, or an
    /// `override_dh_*` style target is debhelper use. Reported when
    /// `Build-Depends`, `Build-Depends-Indep`, and `Build-Depends-Arch`
    /// together imply neither `debhelper` nor `debhelper-compat`; a
    /// dependency restricted by a build profile does not count. Only reached
    /// after a dh command has been seen; a `Build-Driver` other than
    /// `debian-rules` is not scanned. Extra is empty. Location is
    /// `debian/rules` with no line. Applies to the Source tree and Source
    /// package.
    PackageUsesDebhelperButLacksBuildDepends,
    code = "LN0527",
    tag = "package-uses-debhelper-but-lacks-build-depends",
    default_selected = true,
    experimental = false,
}

const BUILD_DEPENDS: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

impl SourceRule for PackageUsesDebhelperButLacksBuildDepends {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((_, scan)) = gated_scan(ctx) else {
            return;
        };
        if !scan.uses_debhelper {
            return;
        }
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let build_depends = control.source().relations_and(BUILD_DEPENDS);
        if build_depends.satisfies("debhelper") || build_depends.satisfies("debhelper-compat") {
            return;
        }
        ctx.diagnostic(Diagnostic::error("").with_location(Location {
            path: PathBuf::from("debian/rules"),
            line: None,
            column: None,
        }));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(build_depends: Option<&str>) -> String {
        let build_depends = build_depends
            .map(|value| format!("Build-Depends: {value}\n"))
            .unwrap_or_default();
        format!(
            "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
{build_depends}\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n"
        )
    }

    const DH: &str = "%:\n\tdh $@\n";

    fn package(build_depends: Option<&str>, rules: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control(build_depends))),
            ("debian/rules", TestFile::new(rules)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package(Some("debhelper (>= 9)"), DH),
            package(Some("debhelper-compat (= 13)"), DH),
            package(Some("debhelper-compat (>= 13)"), DH),
            // No debhelper use.
            package(None, "build:\n\ttrue\n"),
            package(None, "build:\n\t# dh_clean\n"),
            // `dh_autoreconf` alone is not counted as a dh command.
            package(None, "build:\n\tdh_autoreconf\n"),
        ];
        let fail = vec![
            package(None, DH),
            package(Some("foo"), "build:\n\tdh_testroot\n"),
            package(Some("debhelper <!nocheck>"), DH),
            // An alternative does not imply debhelper.
            package(Some("foo, debhelper | bar"), DH),
            package(Some("foo, bar"), "clean:\n\t@dh_clean\n"),
        ];
        SourceTreeTester::new(PackageUsesDebhelperButLacksBuildDepends, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(Some("debhelper"), DH))];
        let fail = vec![SourcePackageCase::from(package(None, DH))];
        SourcePackageTester::new(PackageUsesDebhelperButLacksBuildDepends, pass, fail)
            .test_and_snapshot();
    }
}
