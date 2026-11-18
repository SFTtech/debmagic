use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, RuleMeta, Severity};

use super::engine;

pub struct PrivacyBreachGoogleAdsense;

declare_rule! {
    /// A web page embeds Google AdSense or Analytics.
    ///
    /// Port of lintian's `privacy-breach-google-adsense` from the `files/privacy-breach` Check, which
    /// scans installed `.html`, `.js`, `.css`, `.xml` (and similar) files
    /// against lintian's privacy-breaker tables. Extras and Location follow
    /// lintian. Udebs are checked too.
    PrivacyBreachGoogleAdsense,
    code = "LN0642",
    tag = "privacy-breach-google-adsense",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrivacyBreachGoogleAdsense {
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
            tree(&[(r#"usr/share/example/ok.html"#, r#"<p>adsense</p>"#)]),
        ];
        let fail = vec![
            tree(&[(
                r#"usr/share/example/a.html"#,
                r#"<script>google_ad_client = "pub-1";</script>"#,
            )]),
            tree(&[(
                r#"usr/share/example/b.html"#,
                r#"<script src="http://pagead2.googlesyndication.com/pagead/show_ads.js"></script>"#,
            )]),
            tree(&[(
                r#"usr/share/example/c.js"#,
                r#"var x = "google-analytics.com/ga.js";"#,
            )]),
        ];
        BinaryPackageTester::new(PrivacyBreachGoogleAdsense, pass, fail).test_and_snapshot();
    }
}
