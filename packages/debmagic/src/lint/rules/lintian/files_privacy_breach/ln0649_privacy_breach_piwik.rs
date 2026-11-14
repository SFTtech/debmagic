use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, RuleMeta, Severity};

use super::engine;

pub struct PrivacyBreachPiwik;

declare_rule! {
    /// A web page embeds a Piwik tracker.
    ///
    /// Port of lintian's `privacy-breach-piwik` from the `files/privacy-breach` Check, which
    /// scans installed `.html`, `.js`, `.css`, `.xml` (and similar) files
    /// against lintian's privacy-breaker tables. Extras and Location follow
    /// lintian. Udebs are checked too.
    PrivacyBreachPiwik,
    code = "LN0649",
    tag = "privacy-breach-piwik",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrivacyBreachPiwik {
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
            tree(&[(r#"usr/share/example/ok.html"#, r#"<p>piwik</p>"#)]),
        ];
        let fail = vec![
            tree(&[(
                r#"usr/share/example/a.html"#,
                r#"<img src="http://stats.example.org/piwik.php?idsite=1">"#,
            )]),
            tree(&[(
                r#"usr/share/example/b.html"#,
                r#"<script>var piwik_url = "x";</script>"#,
            )]),
        ];
        BinaryPackageTester::new(PrivacyBreachPiwik, pass, fail).test_and_snapshot();
    }
}
