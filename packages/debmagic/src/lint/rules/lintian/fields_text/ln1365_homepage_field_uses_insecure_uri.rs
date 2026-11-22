use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::data;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity, SourceRule};

pub struct HomepageFieldUsesInsecureUri;

declare_rule! {
    /// `Homepage` is a plain `http://` or `ftp://` URI for a site that serves
    /// HTTPS.
    ///
    /// Reads the source paragraph of `debian/control`, and Installation
    /// control on a Binary package. The unfolded value, without the angle
    /// brackets of an optional `<URL:...>` wrapper, is matched against the
    /// vendored `fields/bad-homepages` expressions for this tag (`ftp://`,
    /// and `http://` for bioconductor.org, cran.r-project.org, debian.org,
    /// github.com, metacpan.org and gnu.org). Extra is the field value as
    /// written, with surrounding space trimmed. Location is empty. Udebs are
    /// checked too. Pedantic.
    HomepageFieldUsesInsecureUri,
    code = "LN1365",
    tag = "homepage-field-uses-insecure-uri",
    default_selected = false,
    experimental = false,
}

const TAG: &str = "homepage-field-uses-insecure-uri";

static CLUTTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^<(?:UR[LI]:)?.*>$").expect("valid regex"));

static EXPRESSIONS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/fields/bad-homepages"))
        .filter_map(|line| line.split_once("~~"))
        .filter(|(tag, _)| tag.trim() == TAG)
        .filter_map(|(_, pattern)| Regex::new(pattern.trim()).ok())
        .collect()
});

impl SourceRule for HomepageFieldUsesInsecureUri {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let messages = matching_homepages(control.source());
        for original in messages {
            ctx.diagnostic(Diagnostic::warn(original).with_severity(Severity::Pedantic));
        }
    }
}

impl BinaryPackageRule for HomepageFieldUsesInsecureUri {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = matching_homepages(ctx.installation_control().fields());
        for original in messages {
            ctx.diagnostic(Diagnostic::warn(original).with_severity(Severity::Pedantic));
        }
    }
}

/// The value as written of every expression that matches the Homepage.
fn matching_homepages(fields: &ControlParagraph) -> Vec<String> {
    if !fields.declares("Homepage") {
        return Vec::new();
    }
    let original = fields
        .untrimmed_value("Homepage")
        .unwrap_or("")
        .trim()
        .to_string();
    let mut homepage = fields.unfolded_value("Homepage").unwrap_or_default();
    if CLUTTER.is_match(&homepage) {
        let mut chars = homepage.chars();
        chars.next();
        chars.next_back();
        homepage = chars.as_str().to_string();
    }
    EXPRESSIONS
        .iter()
        .filter(|regex| regex.is_match(&homepage))
        .map(|_| original.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{
        BinaryPackageCase, BinaryPackageTester, SourcePackageCase, SourcePackageTester, TestFile,
    };

    use super::*;

    fn description(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Homepage", value)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            description("https://example.org/"),
            description("http://example.org/"),
            description("<https://github.com/example/tool>"),
        ];
        let fail = vec![
            description("http://github.com/insecure/uri"),
            description("<http://lintian.debian.org/>"),
            description("http://debian.org/").udeb(),
        ];
        BinaryPackageTester::new(HomepageFieldUsesInsecureUri, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let tree = |homepage: &str| {
            vec![(
                "debian/control",
                TestFile::new(format!(
                    "Source: example\nHomepage: {homepage}\n\nPackage: example\nArchitecture: all\nDescription: x\n y\n"
                )),
            )]
        };
        let pass = vec![SourcePackageCase::from(tree("https://example.org/"))];
        let fail = vec![SourcePackageCase::from(tree(
            "http://github.com/insecure/uri",
        ))];
        SourcePackageTester::new(HomepageFieldUsesInsecureUri, pass, fail).test_and_snapshot();
    }
}
