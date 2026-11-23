use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity, SourceRule};

use super::bad_homepage_matches;

pub struct HomepageForBioconductorPackageNotCanonical;

declare_rule! {
    /// `Homepage` is a Bioconductor package page under `/packages/<release>/bioc/html/`
    /// rather than the canonical `https://bioconductor.org/packages/<name>/`.
    ///
    /// Lintian's `fields/homepage` Check, on the source paragraph and on
    /// Installation control. The Homepage is matched against the vendored
    /// `fields/bad-homepages` expression for this tag, after the optional
    /// `<URL:...>` wrapper is stripped. Extra is the field value as written,
    /// with surrounding space trimmed. Location is empty. Udebs are checked
    /// too.
    HomepageForBioconductorPackageNotCanonical,
    code = "LN1545",
    tag = "homepage-for-bioconductor-package-not-canonical",
    default_selected = false,
    experimental = false,
}

const TAG: &str = "homepage-for-bioconductor-package-not-canonical";

impl SourceRule for HomepageForBioconductorPackageNotCanonical {
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

impl BinaryPackageRule for HomepageForBioconductorPackageNotCanonical {
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

    const HTML: &str = "https://www.bioconductor.org/packages/release/bioc/html/foo.html";

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
            binary("https://bioconductor.org/packages/foo/"),
            binary("https://example.org/bioc/html/foo.html"),
            binary("<https://bioconductor.org/packages/foo/>"),
        ];
        let fail = vec![
            binary(HTML),
            binary(&format!("<{HTML}>")),
            binary(HTML).udeb(),
        ];
        BinaryPackageTester::new(HomepageForBioconductorPackageNotCanonical, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![source("https://bioconductor.org/packages/foo/")];
        let fail = vec![source(HTML)];
        SourcePackageTester::new(HomepageForBioconductorPackageNotCanonical, pass, fail)
            .test_and_snapshot();
    }
}
