use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity, SourceRule};

use super::bad_homepage_matches;

pub struct HomepageSalsaUrlEndsWithDotGit;

declare_rule! {
    /// `Homepage` is a Salsa URL that ends with `.git`.
    HomepageSalsaUrlEndsWithDotGit,
    code = "LN1551",
    tag = "homepage-salsa-url-ends-with-dot-git",
    default_selected = false,
    experimental = false,
}

const TAG: &str = "homepage-salsa-url-ends-with-dot-git";

impl SourceRule for HomepageSalsaUrlEndsWithDotGit {
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

impl BinaryPackageRule for HomepageSalsaUrlEndsWithDotGit {
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
            binary("https://salsa.debian.org/foo/bar"),
            binary("https://salsa.debian.org/foo/bar.git/tree"),
        ];
        let fail = vec![
            binary("https://salsa.debian.org/foo/bar.git"),
            binary("<https://salsa.debian.org/foo/bar.git>"),
            binary("https://salsa.debian.org/foo/bar.git").udeb(),
        ];
        BinaryPackageTester::new(HomepageSalsaUrlEndsWithDotGit, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![source("https://salsa.debian.org/foo/bar")];
        let fail = vec![source("https://salsa.debian.org/foo/bar.git")];
        SourcePackageTester::new(HomepageSalsaUrlEndsWithDotGit, pass, fail).test_and_snapshot();
    }
}
