use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, RuleMeta, Severity};

use super::engine;

pub struct PrivacyBreachFacebook;

declare_rule! {
    /// A web page embeds a Facebook widget.
    ///
    /// Port of lintian's `privacy-breach-facebook` from the `files/privacy-breach` Check, which
    /// scans installed `.html`, `.js`, `.css`, `.xml` (and similar) files
    /// against lintian's privacy-breaker tables. Extras and Location follow
    /// lintian. Udebs are checked too.
    PrivacyBreachFacebook,
    code = "LN0645",
    tag = "privacy-breach-facebook",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrivacyBreachFacebook {
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
            tree(&[(r#"usr/share/example/ok.html"#, r#"<p>facebook</p>"#)]),
            tree(&[(r#"usr/share/example/call.js"#, r#"fb.init({appId: 1});"#)]),
        ];
        let fail = vec![
            tree(&[(
                r#"usr/share/example/a.html"#,
                r#"<script src="http://www.facebook.com/sdk.js"></script>"#,
            )]),
            tree(&[(
                r#"usr/share/example/b.html"#,
                r#"<div id="facebook-jssdk"></div>"#,
            )]),
            tree(&[(r#"usr/share/example/c.js"#, r#"fb.init ;"#)]),
        ];
        BinaryPackageTester::new(PrivacyBreachFacebook, pass, fail).test_and_snapshot();
    }
}
