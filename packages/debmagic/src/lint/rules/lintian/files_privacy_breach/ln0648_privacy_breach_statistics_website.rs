use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, RuleMeta, Severity};

use super::engine;

pub struct PrivacyBreachStatisticsWebsite;

declare_rule! {
    /// A web page embeds a visitor-statistics service.
    ///
    /// Port of lintian's `privacy-breach-statistics-website` from the `files/privacy-breach` Check, which
    /// scans installed `.html`, `.js`, `.css`, `.xml` (and similar) files
    /// against lintian's privacy-breaker tables. Extras and Location follow
    /// lintian. Udebs are checked too.
    PrivacyBreachStatisticsWebsite,
    code = "LN0648",
    tag = "privacy-breach-statistics-website",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrivacyBreachStatisticsWebsite {
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
            tree(&[(r#"usr/share/example/ok.html"#, r#"<p>statcounter</p>"#)]),
        ];
        let fail = vec![
            tree(&[(
                r#"usr/share/example/a.html"#,
                r#"<img src="http://c1.statcounter.com/1/0/1/1/">"#,
            )]),
            tree(&[(
                r#"usr/share/example/b.html"#,
                r#"<script>var sc_project=123;</script>"#,
            )]),
        ];
        BinaryPackageTester::new(PrivacyBreachStatisticsWebsite, pass, fail).test_and_snapshot();
    }
}
