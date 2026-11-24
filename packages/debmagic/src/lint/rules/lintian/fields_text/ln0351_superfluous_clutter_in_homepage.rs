use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

pub struct SuperfluousClutterInHomepage;

declare_rule! {
    /// `Homepage` is wrapped in angle brackets, optionally after `URL:` or `URI:`.
    ///
    /// Installation control on a Binary package. The match is lintian's `/^<(?:UR[LI]:)?.*>$/i` on
    /// the value with newlines removed. Extra is that value with surrounding
    /// space trimmed, brackets included. For a Source tree the source
    /// paragraph of `debian/control` is read, for a Source package the Dsc.
    /// Udebs are checked too.
    SuperfluousClutterInHomepage,
    code = "LN0351",
    tag = "superfluous-clutter-in-homepage",
    default_selected = true,
    experimental = false,
}

static CLUTTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^<(?:UR[LI]:)?.*>$").expect("valid regex"));

impl BinaryPackageRule for SuperfluousClutterInHomepage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if let Some(message) = clutter(ctx.installation_control().fields()) {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

impl SourceRule for SuperfluousClutterInHomepage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let message = match ctx.dsc() {
            Some(dsc) => clutter(dsc.fields()),
            None => ctx
                .debian_control()
                .and_then(|control| clutter(control.source())),
        };
        if let Some(message) = message {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

fn clutter(fields: &ControlParagraph) -> Option<String> {
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
    CLUTTER.is_match(&unfolded).then_some(original)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{
        BinaryPackageCase, BinaryPackageTester, SourcePackageCase, SourcePackageTester,
        SourceTreeTester, TestFile,
    };

    use super::*;

    fn homepage(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Homepage", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            homepage("https://example.com/"),
            homepage("http://lintian.debian.org/"),
            homepage("<http://lintian.debian.org"),
            homepage("http://example.com>"),
            homepage(""),
        ];
        let fail = vec![
            homepage("<http://lintian.debian.org/>"),
            homepage("<URL:https://example.com/>"),
            homepage("<URI:https://example.com/>"),
            homepage("<url:https://example.com/>"),
            homepage("<not-a-url>"),
            homepage("<>"),
            homepage("<http://lintian.debian.org/>").udeb(),
        ];
        BinaryPackageTester::new(SuperfluousClutterInHomepage, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }

    fn source(homepage: &str) -> Vec<(String, TestFile)> {
        let field = if homepage.is_empty() {
            String::new()
        } else {
            format!("Homepage: {homepage}\n")
        };
        vec![(
            "debian/control".to_string(),
            TestFile::from(format!(
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n\
                 {field}\n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            source(""),
            source("https://example.com/"),
            source("<http://lintian.debian.org"),
        ];
        let fail = vec![
            source("<http://lintian.debian.org/>"),
            source("<insert the upstream URL, if relevant>"),
        ];
        SourceTreeTester::new(SuperfluousClutterInHomepage, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(source("https://example.com/"))];
        let fail = vec![SourcePackageCase::from(source(
            "<URL:https://example.com/>",
        ))];
        SourcePackageTester::new(SuperfluousClutterInHomepage, pass, fail).test_and_snapshot();
    }
}
