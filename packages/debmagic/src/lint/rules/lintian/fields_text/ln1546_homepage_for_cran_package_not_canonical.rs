use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity, SourceRule};

use super::bad_homepage_matches;

pub struct HomepageForCranPackageNotCanonical;

declare_rule! {
    /// `Homepage` is a CRAN package page under `/web/packages/` rather than
    /// the canonical `https://cran.r-project.org/package=<name>`.
    HomepageForCranPackageNotCanonical,
    code = "LN1546",
    tag = "homepage-for-cran-package-not-canonical",
    default_selected = false,
    experimental = false,
}

const TAG: &str = "homepage-for-cran-package-not-canonical";

impl SourceRule for HomepageForCranPackageNotCanonical {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let messages = bad_homepage_matches(TAG, control.source());
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message).with_severity(Severity::Info));
        }
    }
}

impl BinaryPackageRule for HomepageForCranPackageNotCanonical {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = bad_homepage_matches(TAG, ctx.installation_control().fields());
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message).with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{
        BinaryPackageCase, BinaryPackageTester, SourcePackageCase, SourcePackageTester, TestFile,
    };

    use super::*;

    fn binary(homepage: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Homepage", homepage)
    }

    fn source(homepage: &str) -> SourcePackageCase {
        SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nHomepage: {homepage}\n\nPackage: example\nArchitecture: all\nDescription: x\n y\n"
            )),
        )])
    }

    #[test]
    fn binary_package() {
        let pass = vec![
            binary("https://cran.r-project.org/package=foo"),
            binary("https://example.org/web/packages/foo/"),
        ];
        let fail = vec![
            binary("https://cran.r-project.org/web/packages/foo/index.html"),
            binary("<https://cran.r-project.org/web/packages/foo/index.html>"),
            binary("https://cran.r-project.org/web/packages/foo/index.html").udeb(),
        ];
        BinaryPackageTester::new(HomepageForCranPackageNotCanonical, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![source("https://cran.r-project.org/package=foo")];
        let fail = vec![source(
            "https://cran.r-project.org/web/packages/foo/index.html",
        )];
        SourcePackageTester::new(HomepageForCranPackageNotCanonical, pass, fail)
            .test_and_snapshot();
    }
}
