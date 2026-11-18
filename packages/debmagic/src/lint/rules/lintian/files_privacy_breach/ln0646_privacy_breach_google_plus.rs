use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, RuleMeta, Severity};

use super::engine;

pub struct PrivacyBreachGooglePlus;

declare_rule! {
    /// A web page embeds a Google+ button.
    ///
    /// Port of lintian's `privacy-breach-google-plus` from the `files/privacy-breach` Check, which
    /// scans installed `.html`, `.js`, `.css`, `.xml` (and similar) files
    /// against lintian's privacy-breaker tables. Extras and Location follow
    /// lintian. Udebs are checked too.
    PrivacyBreachGooglePlus,
    code = "LN0646",
    tag = "privacy-breach-google-plus",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrivacyBreachGooglePlus {
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
            tree(&[(r#"usr/share/example/ok.html"#, r#"<p>plusone</p>"#)]),
        ];
        let fail = vec![
            tree(&[(
                r#"usr/share/example/a.html"#,
                r#"<script src="http://apis.google.com/js/plusone.js"></script>"#,
            )]),
            tree(&[(r#"usr/share/example/b.html"#, r#"<g:plusone></g:plusone>"#)]),
        ];
        BinaryPackageTester::new(PrivacyBreachGooglePlus, pass, fail).test_and_snapshot();
    }
}
