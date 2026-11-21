use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::SubjectKind;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct RubygemHomepage;

declare_rule! {
    /// The `Homepage` field points at Rubygems (`rubygems.org/gems/...`)
    /// rather than at the upstream project.
    ///
    /// Read from the source paragraph of `debian/control` on a Source tree, or
    /// from the `.dsc` on a Source package. The match is case-insensitive and
    /// anchored at the start of the value, with an optional `www.` and an
    /// `http` or `https` scheme. Extra is the whole value. A value with a line
    /// break is unfolded to one line first. No Location.
    RubygemHomepage,
    code = "LN1121",
    tag = "rubygem-homepage",
    default_selected = true,
    experimental = false,
}

/// lintian's `m{^http s? :// (?:www [.])? rubygems [.] org/gems/}isx`, where
/// `s` lets `.` match newlines and does not change the anchored start.
static RUBYGEMS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^https?://(?:www\.)?rubygems\.org/gems/").expect("valid regex")
});

impl SourceRule for RubygemHomepage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let homepage = match ctx.kind() {
            SubjectKind::SourceTree => ctx
                .debian_control()
                .and_then(|control| control.source().unfolded_value("Homepage")),
            SubjectKind::SourcePackage => ctx
                .dsc()
                .and_then(|dsc| dsc.fields().unfolded_value("Homepage")),
            SubjectKind::BinaryPackage => None,
        };
        if let Some(homepage) = homepage.filter(|homepage| RUBYGEMS.is_match(homepage)) {
            ctx.diagnostic(Diagnostic::warn(homepage));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestCase};

    use super::*;

    fn control(homepage: &str) -> Vec<(&'static str, String)> {
        vec![(
            "debian/control",
            format!(
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 Homepage: {homepage}\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n"
            ),
        )]
    }

    #[test]
    fn source() {
        let pass = vec![
            TestCase::from(control("https://github.com/example/some-gem")),
            TestCase::from(control("")),
            TestCase::from(vec![(
                "debian/control",
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Homepage: https://rubygems.org/gems/some-gem/\n\
                 Description: example\n extra\n"
                    .to_string(),
            )]),
        ];
        let fail = vec![
            TestCase::from(control("https://rubygems.org/gems/some-gem/")),
            TestCase::from(control("http://www.RubyGems.org/gems/some-gem")),
        ];
        SourceTreeTester::new(RubygemHomepage, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let dsc = |homepage: &str| {
            crate::lint::dsc::Dsc::parse(&format!(
                "Format: 3.0 (native)\nSource: example\nVersion: 1.0\nHomepage: {homepage}\n"
            ))
            .expect("Dsc fixture")
        };
        let pass = vec![
            SourcePackageCase::from(TestCase::from(control("https://github.com/example/x")))
                .dsc(dsc("https://github.com/example/x")),
            SourcePackageCase::from(TestCase::from(control("https://rubygems.org/gems/x")))
                .dsc(dsc("https://github.com/example/x")),
        ];
        let fail = vec![
            SourcePackageCase::from(TestCase::from(control("https://github.com/example/x")))
                .dsc(dsc("https://rubygems.org/gems/some-gem/")),
        ];
        SourcePackageTester::new(RubygemHomepage, pass, fail).test_and_snapshot();
    }
}
