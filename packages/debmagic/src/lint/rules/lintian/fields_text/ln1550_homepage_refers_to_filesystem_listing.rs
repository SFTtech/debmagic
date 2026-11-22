use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity, SourceRule};

use super::bad_homepage_matches;

pub struct HomepageRefersToFilesystemListing;

declare_rule! {
    /// `Homepage` points at a download directory or filesystem listing on
    /// `ftp.gnu.org`, rather than the project's homepage.
    HomepageRefersToFilesystemListing,
    code = "LN1550",
    tag = "homepage-refers-to-filesystem-listing",
    default_selected = false,
    experimental = false,
}

const TAG: &str = "homepage-refers-to-filesystem-listing";

impl SourceRule for HomepageRefersToFilesystemListing {
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

impl BinaryPackageRule for HomepageRefersToFilesystemListing {
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
            binary("https://www.gnu.org/software/aspell/"),
            binary("https://ftp.gnu.org.example/"),
        ];
        let fail = vec![
            binary("http://ftp.gnu.org/gnu/aspell/dict/ar/"),
            binary("<https://ftp.gnu.org/gnu/aspell/>"),
            binary("http://ftp.gnu.org/gnu/aspell/dict/ar/").udeb(),
        ];
        BinaryPackageTester::new(HomepageRefersToFilesystemListing, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![source("https://www.gnu.org/software/aspell/")];
        let fail = vec![source("http://ftp.gnu.org/gnu/aspell/dict/ar/")];
        SourcePackageTester::new(HomepageRefersToFilesystemListing, pass, fail).test_and_snapshot();
    }
}
