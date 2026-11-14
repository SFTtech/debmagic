use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

pub struct HomepageForCpanPackageContainsVersion;

declare_rule! {
    /// `Homepage` points at a versioned CPAN or MetaCPAN URL.
    ///
    /// The source paragraph and Installation control. Angle brackets (optional
    /// `URL:` / `URI:` inside) are stripped before the match. Extra is the
    /// field value with surrounding space trimmed. Udebs are checked too.
    HomepageForCpanPackageContainsVersion,
    code = "LN0436",
    tag = "homepage-for-cpan-package-contains-version",
    default_selected = true,
    experimental = false,
}

static CLUTTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^<(?:UR[LI]:)?.*>$").expect("valid regex"));
static CPAN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:search\.cpan\.org|metacpan\.org)/.*-[0-9._]+/*").expect("valid regex")
});

impl SourceRule for HomepageForCpanPackageContainsVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        if let Some(message) = cpan_version(control.source()) {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

impl BinaryPackageRule for HomepageForCpanPackageContainsVersion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if let Some(message) = cpan_version(ctx.installation_control().fields()) {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

fn cpan_version(fields: &ControlParagraph) -> Option<String> {
    let (original, unfolded) = prepared(fields)?;
    CPAN.is_match(&unfolded).then_some(original)
}

pub(super) fn prepared(fields: &ControlParagraph) -> Option<(String, String)> {
    if !fields.declares("Homepage") {
        return None;
    }
    let original = fields
        .untrimmed_value("Homepage")
        .unwrap_or("")
        .trim()
        .to_string();
    let mut unfolded = original.replace('\n', "");
    unfolded = unfolded.trim_start().to_string();
    if CLUTTER.is_match(&unfolded) {
        let mut chars = unfolded.chars();
        chars.next();
        chars.next_back();
        unfolded = chars.as_str().to_string();
    }
    Some((original, unfolded))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{
        BinaryPackageCase, BinaryPackageTester, SourcePackageCase, SourcePackageTester,
        SourceTreeTester, TestFile,
    };

    use super::*;

    fn control(homepage: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nHomepage: {homepage}\n\nPackage: example\nArchitecture: all\nDescription: d\n more\n"
            )),
        )]
    }

    fn deb(homepage: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Homepage", homepage)
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            control("https://metacpan.org/release/HTML-Template"),
            control("http://search.cpan.org/dist/HTML-Template/"),
            control("https://example.com/Foo-1.0/"),
        ];
        let fail = vec![
            control("http://search.cpan.org/Foo-Bar-9.0_01/"),
            control("https://metacpan.org/release/Foo-1.2"),
            control("<http://search.cpan.org/~sam/HTML-Template-2.9/>"),
        ];
        SourceTreeTester::new(HomepageForCpanPackageContainsVersion, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(control(
            "https://metacpan.org/release/HTML-Template",
        ))];
        let fail = vec![SourcePackageCase::from(control(
            "http://search.cpan.org/Foo-Bar-9.0_01/",
        ))];
        SourcePackageTester::new(HomepageForCpanPackageContainsVersion, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn binary() {
        let pass = vec![deb("https://example.com/")];
        let fail = vec![
            deb("http://search.cpan.org/Foo-Bar-9.0_01/"),
            deb("https://metacpan.org/release/Foo-1.2").udeb(),
        ];
        BinaryPackageTester::new(HomepageForCpanPackageContainsVersion, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
