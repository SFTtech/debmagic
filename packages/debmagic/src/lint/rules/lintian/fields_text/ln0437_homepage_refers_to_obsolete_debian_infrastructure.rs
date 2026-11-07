use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::ln0436_homepage_for_cpan_package_contains_version::prepared;

pub struct HomepageRefersToObsoleteDebianInfrastructure;

declare_rule! {
    /// `Homepage` points at `alioth.debian.org`.
    ///
    /// The source paragraph and Installation control. Angle brackets (optional
    /// `URL:` / `URI:` inside) are stripped before the match. Extra is the
    /// field value with surrounding space trimmed. Udebs are checked too.
    HomepageRefersToObsoleteDebianInfrastructure,
    code = "LN0437",
    tag = "homepage-refers-to-obsolete-debian-infrastructure",
    default_selected = true,
    experimental = false,
}

static ALIOTH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"alioth\.debian\.org").expect("valid regex"));

impl SourceRule for HomepageRefersToObsoleteDebianInfrastructure {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        if let Some(message) = alioth(control.source()) {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

impl BinaryPackageRule for HomepageRefersToObsoleteDebianInfrastructure {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if let Some(message) = alioth(ctx.installation_control().fields()) {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

fn alioth(fields: &ControlParagraph) -> Option<String> {
    let (original, unfolded) = prepared(fields)?;
    ALIOTH.is_match(&unfolded).then_some(original)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, SourceTreeTester, TestFile};

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
            control("https://salsa.debian.org/foo"),
            control("https://example.com/"),
        ];
        let fail = vec![
            control("https://alioth.debian.org/foo/"),
            control("http://alioth.debian.org/"),
        ];
        SourceTreeTester::new(HomepageRefersToObsoleteDebianInfrastructure, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn binary() {
        let pass = vec![deb("https://salsa.debian.org/foo")];
        let fail = vec![
            deb("https://alioth.debian.org/foo/"),
            deb("https://alioth.debian.org/foo/").udeb(),
        ];
        BinaryPackageTester::new(HomepageRefersToObsoleteDebianInfrastructure, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
