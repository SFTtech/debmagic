use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, RuleMeta, Severity};

use super::engine;

pub struct PrivacyBreachDonation;

declare_rule! {
    /// A web page embeds a donation button from a remote site.
    ///
    /// Port of lintian's `privacy-breach-donation` from the `files/privacy-breach` Check, which
    /// scans installed `.html`, `.js`, `.css`, `.xml` (and similar) files
    /// against lintian's privacy-breaker tables. Extras and Location follow
    /// lintian. Udebs are checked too.
    PrivacyBreachDonation,
    code = "LN0643",
    tag = "privacy-breach-donation",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrivacyBreachDonation {
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
            tree(&[(r#"usr/share/example/ok.html"#, r#"<img src="donate.png">"#)]),
        ];
        let fail = vec![
            tree(&[(
                r#"usr/share/example/a.html"#,
                r#"<img src="http://example.org/donate.png">"#,
            )]),
            tree(&[(
                r#"usr/share/example/b.html"#,
                r#"<a href="x"><img src="https://www.paypal.com/en_US/i/btn/btn_donate.gif"></a>"#,
            )]),
            tree(&[(
                r#"usr/share/example/c.html"#,
                r#"<a class="flattrbutton" href="x">flattr</a>"#,
            )]),
        ];
        BinaryPackageTester::new(PrivacyBreachDonation, pass, fail).test_and_snapshot();
    }
}
