use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::{is_pkg_perl, source_fields};

pub struct NoTestsuiteHeader;

declare_rule! {
    /// A pkg-perl package declares no `Testsuite` field.
    ///
    /// The Maintainer must contain `pkg-perl-maintainers@lists.alioth.debian.org`
    /// (substring). Source package: the `.dsc` fields; Source tree: the source
    /// paragraph of `debian/control`. Extra and Location are empty. Whether the
    /// field names `autopkgtest-pkg-perl` is not checked here (that is
    /// `no-team-tests`, a Tag this Rule does not emit).
    NoTestsuiteHeader,
    code = "LN1782",
    tag = "no-testsuite-header",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NoTestsuiteHeader {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let report = {
            let Some(fields) = source_fields(ctx) else {
                return;
            };
            is_pkg_perl(fields) && !fields.declares("Testsuite")
        };
        if report {
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
            Vec::<(&str, TestFile)>::new(),
            control("Someone <someone@example.com>", None),
            control(PKG_PERL, Some("autopkgtest-pkg-perl")),
            control(PKG_PERL, Some("autopkgtest, autopkgtest-pkg-perl")),
        ];
        let fail = vec![control(PKG_PERL, None)];
        SourceTreeTester::new(NoTestsuiteHeader, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let bare = Vec::<(&str, TestFile)>::new();
        let pass = vec![
            SourcePackageCase::from(bare.clone()).dsc(dsc("Someone <someone@example.com>", None)),
            SourcePackageCase::from(bare.clone()).dsc(dsc(PKG_PERL, Some("autopkgtest-pkg-perl"))),
        ];
        let fail = vec![SourcePackageCase::from(bare).dsc(dsc(PKG_PERL, None))];
        SourcePackageTester::new(NoTestsuiteHeader, pass, fail).test_and_snapshot();
    }
}
