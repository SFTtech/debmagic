use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::super::team_pkg_perl::{is_pkg_perl, source_fields};

pub struct NoTeamTests;

declare_rule! {
    /// A pkg-perl package (its Maintainer names `pkg-perl-maintainers@lists.alioth.debian.org`)
    /// declares `Testsuite` without `autopkgtest-pkg-perl` in its comma-separated
    /// list. The package should use `Testsuite: autopkgtest-pkg-perl` instead of
    /// a copy of the standard test control file. Extra is empty.
    ///
    /// Lintian's `team/pkg-perl/testsuite` Check. Reads the Dsc, or the source
    /// paragraph of `debian/control` on a Source tree. An undeclared `Testsuite`
    /// is left to the Check's `no-testsuite-header` Tag and is not reported here.
    /// Location is empty.
    NoTeamTests,
    code = "LN1786",
    tag = "no-team-tests",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NoTeamTests {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let flagged = {
            let Some(fields) = source_fields(ctx) else {
                return;
            };
            if !is_pkg_perl(fields) {
                return;
            }
            fields.unfolded_value("Testsuite").is_some_and(|testsuite| {
                !testsuite
                    .split(',')
                    .any(|name| name.trim() == "autopkgtest-pkg-perl")
            })
        };
        if flagged {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const PKG_PERL: &str = "Debian Perl Group <pkg-perl-maintainers@lists.alioth.debian.org>";

    fn control(maintainer: &str, testsuite: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let field = testsuite
            .map(|value| format!("Testsuite: {value}\n"))
            .unwrap_or_default();
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: libfoo-perl\nMaintainer: {maintainer}\n{field}\
                 Standards-Version: 4.7.2\nSection: perl\n\
                 \nPackage: libfoo-perl\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    fn dsc(maintainer: &str, testsuite: Option<&str>) -> Dsc {
        let field = testsuite
            .map(|value| format!("Testsuite: {value}\n"))
            .unwrap_or_default();
        Dsc::parse(&format!(
            "Source: libfoo-perl\nMaintainer: {maintainer}\n{field}"
        ))
        .expect("Dsc fixture")
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            control(PKG_PERL, Some("autopkgtest-pkg-perl")),
            control(PKG_PERL, Some("autopkgtest, autopkgtest-pkg-perl")),
            // No Testsuite is the no-testsuite-header Tag, not this one.
            control(PKG_PERL, None),
            // Not a pkg-perl package.
            control("Foo <foo@example.com>", Some("autopkgtest")),
            Vec::<(&str, TestFile)>::new(),
            // A non-UTF-8 debian/control is not read at all.
            vec![(
                "debian/control",
                TestFile::from_bytes(
                    b"Source: libfoo-perl\nMaintainer: \xff\nTestsuite: autopkgtest\n".to_vec(),
                ),
            )],
        ];
        let fail = vec![
            control(PKG_PERL, Some("autopkgtest")),
            control(PKG_PERL, Some("autopkgtest, autopkgtest-pkg-perl-extra")),
            // Declared but empty names no autopkgtest-pkg-perl.
            control(PKG_PERL, Some("")),
        ];
        SourceTreeTester::new(NoTeamTests, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let bare = Vec::<(&str, TestFile)>::new();
        let pass = vec![
            SourcePackageCase::from(bare.clone()).dsc(dsc(PKG_PERL, Some("autopkgtest-pkg-perl"))),
            SourcePackageCase::from(bare.clone()).dsc(dsc(PKG_PERL, None)),
            SourcePackageCase::from(bare.clone())
                .dsc(dsc("Foo <foo@example.com>", Some("autopkgtest"))),
        ];
        let fail = vec![
            SourcePackageCase::from(bare.clone()).dsc(dsc(PKG_PERL, Some("autopkgtest"))),
            SourcePackageCase::from(bare).dsc(dsc(PKG_PERL, Some("autopkgtest-pkg-perl-extra"))),
        ];
        SourcePackageTester::new(NoTeamTests, pass, fail).test_and_snapshot();
    }
}
