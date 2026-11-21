use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, RuleMeta, Severity};

use super::engine;

pub struct PrivacyBreachW3cValidHtml;

declare_rule! {
    /// A web page fetches a W3C validation icon or links to the W3C validator.
    ///
    /// Port of lintian's `privacy-breach-w3c-valid-html` from the `files/privacy-breach` Check, which
    /// scans installed `.html`, `.js`, `.css`, `.xml` (and similar) files
    /// against lintian's privacy-breaker tables. Extras and Location follow
    /// lintian. Udebs are checked too.
    PrivacyBreachW3cValidHtml,
    code = "LN1084",
    tag = "privacy-breach-w3c-valid-html",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrivacyBreachW3cValidHtml {
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
            tree(&[(r#"usr/share/example/ok.html"#, r#"<p>w3c</p>"#)]),
            tree(&[(
                r#"usr/share/example/other.html"#,
                r#"<img src="http://www.example.org/icons/valid-xhtml10">"#,
            )]),
        ];
        let fail = vec![
            tree(&[(
                r#"usr/share/example/a.html"#,
                r#"<img src="http://www.w3.org/icons/valid-xhtml10" alt="Valid XHTML 1.0">"#,
            )]),
            tree(&[(
                r#"usr/share/example/b.html"#,
                r#"<img src="http://validator.w3.org/check?uri=referer">"#,
            )]),
            tree(&[(
                r#"usr/share/example/c.html"#,
                r#"<img src="//www.w3.org/css-validator/images/vcss">"#,
            )]),
        ];
        BinaryPackageTester::new(PrivacyBreachW3cValidHtml, pass, fail).test_and_snapshot();
    }
}
