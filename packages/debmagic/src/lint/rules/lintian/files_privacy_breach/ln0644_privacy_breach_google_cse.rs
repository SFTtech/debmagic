use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, RuleMeta, Severity};

use super::engine;

pub struct PrivacyBreachGoogleCse;

declare_rule! {
    /// A web page embeds a Google Custom Search Engine.
    ///
    /// Port of lintian's `privacy-breach-google-cse` from the `files/privacy-breach` Check, which
    /// scans installed `.html`, `.js`, `.css`, `.xml` (and similar) files
    /// against lintian's privacy-breaker tables. Extras and Location follow
    /// lintian. Udebs are checked too.
    PrivacyBreachGoogleCse,
    code = "LN0644",
    tag = "privacy-breach-google-cse",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrivacyBreachGoogleCse {
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
            tree(&[(r#"usr/share/example/ok.html"#, r#"<p>search</p>"#)]),
        ];
        let fail = vec![
            tree(&[(
                r#"usr/share/example/a.html"#,
                r#"<script>googlesearchdomain = "www.google.com";</script>"#,
            )]),
            tree(&[(
                r#"usr/share/example/b.html"#,
                r#"<div class="cse-branding-logo"></div>"#,
            )]),
        ];
        BinaryPackageTester::new(PrivacyBreachGoogleCse, pass, fail).test_and_snapshot();
    }
}
