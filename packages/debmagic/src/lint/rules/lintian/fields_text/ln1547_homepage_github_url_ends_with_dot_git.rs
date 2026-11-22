use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity, SourceRule};

use super::bad_homepage_matches;

pub struct HomepageGithubUrlEndsWithDotGit;

declare_rule! {
    /// `Homepage` is a GitHub URL that ends with `.git`.
    HomepageGithubUrlEndsWithDotGit,
    code = "LN1547",
    tag = "homepage-github-url-ends-with-dot-git",
    default_selected = false,
    experimental = false,
}

const TAG: &str = "homepage-github-url-ends-with-dot-git";

impl SourceRule for HomepageGithubUrlEndsWithDotGit {
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

impl BinaryPackageRule for HomepageGithubUrlEndsWithDotGit {
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
            binary("https://github.com/foo/bar"),
            binary("https://github.com/foo/bar.git/tree"),
        ];
        let fail = vec![
            binary("https://github.com/foo/bar.git"),
            binary("<https://github.com/foo/bar.git>"),
            binary("https://github.com/foo/bar.git").udeb(),
        ];
        BinaryPackageTester::new(HomepageGithubUrlEndsWithDotGit, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![source("https://github.com/foo/bar")];
        let fail = vec![source("https://github.com/foo/bar.git")];
        SourcePackageTester::new(HomepageGithubUrlEndsWithDotGit, pass, fail).test_and_snapshot();
    }
}
