use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, RuleMeta, Severity};

use super::engine;

pub struct PrivacyBreachGeneric;

declare_rule! {
    /// A web page references a resource on an external site that no table entry recognises.
    ///
    /// Port of lintian's `privacy-breach-generic` from the `files/privacy-breach` Check, which
    /// scans installed `.html`, `.js`, `.css`, `.xml` (and similar) files
    /// against lintian's privacy-breaker tables. Extras and Location follow
    /// lintian. Udebs are checked too.
    PrivacyBreachGeneric,
    code = "LN0640",
    tag = "privacy-breach-generic",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrivacyBreachGeneric {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        engine::report(ctx, Self::TAG.0, Severity::Warning);
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
            tree(&[(r#"usr/share/example/ok.html"#, r#"<img src="pixel.png">"#)]),
            tree(&[(
                r#"usr/share/example/local.html"#,
                r#"<img src="http://localhost/pixel.png">"#,
            )]),
            tree(&[(
                r#"usr/share/example/loopback.html"#,
                r#"<img src="http://127.0.0.1:8080/pixel.png">"#,
            )]),
            tree(&[(
                r#"usr/share/example/rel.html"#,
                r#"<link rel="author" href="http://tracker.example/me">"#,
            )]),
            tree(&[(
                r#"usr/share/example/comment.html"#,
                r#"<!-- <img src="http://tracker.example/pixel.png"> -->"#,
            )]),
            tree(&[(
                r#"usr/share/example/other.txt"#,
                r#"<img src="http://tracker.example/pixel.png">"#,
            )]),
        ];
        let fail = vec![
            tree(&[(
                r#"usr/share/example/a.html"#,
                r#"<img src="http://tracker.example/pixel.png">"#,
            )]),
            tree(&[(
                r#"usr/share/example/b.css"#,
                r#"@import url("http://tracker.example/t.css");"#,
            )]),
            tree(&[(
                r#"usr/share/example/c.html"#,
                r#"<link rel="stylesheet" href="//tracker.example/s.css">"#,
            )]),
            tree(&[(
                r#"usr/share/example/d.html"#,
                r#"<!--[if IE]><script src="http://tracker.example/t.js"></script><![endif]-->"#,
            )]),
            tree(&[(
                r#"usr/share/example/multi.html"#,
                "<link href=\"http://tracker.example/s.css\" rel=\"stylesheet\"\n type=\"text/css\">",
            )]),
            tree(&[(
                r#"usr/share/example/e.html"#,
                r#"<img src="http://tracker.example/a.png"><img src="http://tracker.example/b.png"><img src="http://other.example/b.png">"#,
            )]),
        ];
        BinaryPackageTester::new(PrivacyBreachGeneric, pass, fail).test_and_snapshot();
    }
}
