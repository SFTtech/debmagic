use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, RuleMeta, Severity};

use super::engine;

pub struct PrivacyBreachTwitter;

declare_rule! {
    /// A web page embeds the Twitter widgets script.
    ///
    /// Port of lintian's `privacy-breach-twitter` from the `files/privacy-breach` Check, which
    /// scans installed `.html`, `.js`, `.css`, `.xml` (and similar) files
    /// against lintian's privacy-breaker tables. Extras and Location follow
    /// lintian. Udebs are checked too.
    PrivacyBreachTwitter,
    code = "LN0650",
    tag = "privacy-breach-twitter",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrivacyBreachTwitter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        engine::report(ctx, Self::TAG.0, Severity::Error);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: &[(&str, &str)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .iter()
                .map(|(path, content)| (path.to_string(), TestFile::new(*content)))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&[(r#"usr/share/example/ok.html"#, r#"<p>twitter</p>"#)]),
        ];
        let fail = vec![
            tree(&[(
                r#"usr/share/example/a.html"#,
                r#"<script src="https://platform.twitter.com/widgets.js"></script>"#,
            )]),
            tree(&[(
                r#"usr/share/example/b.js"#,
                r#"var s = '<script src=' + '://platform.twitter.com/widgets.js"></script>';"#,
            )]),
        ];
        BinaryPackageTester::new(PrivacyBreachTwitter, pass, fail).test_and_snapshot();
    }
}
