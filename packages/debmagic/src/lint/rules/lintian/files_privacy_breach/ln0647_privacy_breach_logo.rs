use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, RuleMeta, Severity};

use super::engine;

pub struct PrivacyBreachLogo;

declare_rule! {
    /// A web page hot-links a logo or icon from a remote site.
    ///
    /// Port of lintian's `privacy-breach-logo` from the `files/privacy-breach` Check, which
    /// scans installed `.html`, `.js`, `.css`, `.xml` (and similar) files
    /// against lintian's privacy-breaker tables. Extras and Location follow
    /// lintian. Udebs are checked too.
    PrivacyBreachLogo,
    code = "LN0647",
    tag = "privacy-breach-logo",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrivacyBreachLogo {
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
            tree(&[(r#"usr/share/example/ok.html"#, r#"<img src="logo.png">"#)]),
        ];
        let fail = vec![
            tree(&[(
                r#"usr/share/example/a.html"#,
                r#"<img src="http://example.org/logo.png">"#,
            )]),
            tree(&[(
                r#"usr/share/example/b.html"#,
                r#"<img src="http://example.org/images/logos/a.png">"#,
            )]),
        ];
        BinaryPackageTester::new(PrivacyBreachLogo, pass, fail).test_and_snapshot();
    }
}
