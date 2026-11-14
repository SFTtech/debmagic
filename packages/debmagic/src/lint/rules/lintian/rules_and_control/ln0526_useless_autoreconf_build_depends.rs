use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::dh_scan::gated_scan;

pub struct UselessAutoreconfBuildDepends;

declare_rule! {
    /// A build dependency on `dh-autoreconf` or `autotools-dev` is no longer
    /// needed.
    ///
    /// Reported when the declared compat level is 10 or newer and
    /// `Build-Depends`, `Build-Depends-Indep`, and `Build-Depends-Arch`
    /// together imply `dh-autoreconf`, and again for `autotools-dev` unless
    /// `debian/rules` still uses the deprecated autotools-dev debhelper tools
    /// (`dh_autotools-dev_updateconfig`, `dh_autotools-dev_restoreconfig`, or
    /// the `autotools-dev` addon). Only reached after a dh command has been
    /// seen; a `Build-Driver` other than `debian-rules` is not scanned. Extra
    /// is `(does not need to satisfy` the package and `:any)`. There is no
    /// Location. Applies to the Source tree and Source package.
    UselessAutoreconfBuildDepends,
    code = "LN0526",
    tag = "useless-autoreconf-build-depends",
    default_selected = true,
    experimental = false,
}

/// lintian 2.139.0 `$USES_AUTORECONF`.
const USES_AUTORECONF: u32 = 10;

const BUILD_DEPENDS: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

impl SourceRule for UselessAutoreconfBuildDepends {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((facts, scan)) = gated_scan(ctx) else {
            return;
        };
        if facts.level() < USES_AUTORECONF {
            return;
        }
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let build_depends = control.source().relations_and(BUILD_DEPENDS);
        let mut found = Vec::new();
        for package in ["dh-autoreconf", "autotools-dev"] {
            if package == "autotools-dev" && scan.uses_autotools_dev_dh {
                continue;
            }
            if build_depends.satisfies(package) {
                found.push(package);
            }
        }
        for package in found {
            ctx.diagnostic(Diagnostic::warn(format!(
                "(does not need to satisfy {package}:any)"
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(build_depends: &str) -> String {
        format!(
            "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
Build-Depends: {build_depends}\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n"
        )
    }

    const DH: &str = "%:\n\tdh $@\n";

    fn package(build_depends: &str, rules: &str, compat: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control(build_depends))),
            ("debian/rules", TestFile::new(rules)),
            ("debian/compat", TestFile::new(compat)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package("debhelper", DH, "10\n"),
            // Compat 9 still needs them.
            package("debhelper, dh-autoreconf, autotools-dev", DH, "9\n"),
            // The deprecated tools still need autotools-dev, but not
            // dh-autoreconf.
            package(
                "debhelper, autotools-dev",
                "%:\n\tdh $@\n\tdh_autotools-dev_updateconfig\n",
                "12\n",
            ),
            package(
                "debhelper, autotools-dev",
                "%:\n\tdh $@ --with autotools-dev\n",
                "12\n",
            ),
            // Only a profile-restricted dependency is no implication.
            package("debhelper, dh-autoreconf <!nocheck>", DH, "12\n"),
            // A substring is another package.
            package("debhelper, dh-autoreconf-extra", DH, "12\n"),
        ];
        let fail = vec![
            package("debhelper, dh-autoreconf", DH, "10\n"),
            package("debhelper, autotools-dev", DH, "11\n"),
            package(
                "debhelper, autotools-dev, dh-autoreconf (>= 12)",
                DH,
                "13\n",
            ),
            package(
                "debhelper, autotools-dev, dh-autoreconf",
                "%:\n\tdh $@\n\tdh_autotools-dev_restoreconfig\n",
                "12\n",
            ),
        ];
        SourceTreeTester::new(UselessAutoreconfBuildDepends, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            "debhelper, dh-autoreconf",
            DH,
            "9\n",
        ))];
        let fail = vec![SourcePackageCase::from(package(
            "debhelper, dh-autoreconf",
            DH,
            "10\n",
        ))];
        SourcePackageTester::new(UselessAutoreconfBuildDepends, pass, fail).test_and_snapshot();
    }
}
