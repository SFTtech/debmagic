use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, RuleMeta, Severity};

use super::engine;

pub struct PrivacyBreachUsesEmbeddedFile;

declare_rule! {
    /// A web page loads a library that Debian packages (jQuery, Bootstrap, Mocha, ...) from a remote site.
    ///
    /// Port of lintian's `privacy-breach-uses-embedded-file` from the `files/privacy-breach` Check, which
    /// scans installed `.html`, `.js`, `.css`, `.xml` (and similar) files
    /// against lintian's privacy-breaker tables. Extras and Location follow
    /// lintian. Udebs are checked too.
    PrivacyBreachUsesEmbeddedFile,
    code = "LN0641",
    tag = "privacy-breach-uses-embedded-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrivacyBreachUsesEmbeddedFile {
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
            tree(&[(
                r#"usr/share/example/ok.html"#,
                r#"<script src="jquery.js"></script>"#,
            )]),
            tree(&[(
                r#"usr/share/example/local.html"#,
                r#"<script src="http://localhost/jquery.js"></script>"#,
            )]),
        ];
        let fail = vec![
            tree(&[(
                r#"usr/share/example/a.html"#,
                r#"<script src="http://code.jquery.com/jquery-1.8.js"></script>"#,
            )]),
            tree(&[(
                r#"usr/share/example/b.html"#,
                r#"<link rel="stylesheet" href="//cdnjs.cloudflare.com/ajax/libs/mocha/2.1.0/mocha.min.css">"#,
            )]),
            tree(&[(
                r#"usr/share/example/c.html"#,
                r#"<script src="https://example.org/html5shiv.js"></script>"#,
            )]),
        ];
        BinaryPackageTester::new(PrivacyBreachUsesEmbeddedFile, pass, fail).test_and_snapshot();
    }
}
