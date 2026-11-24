use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_text;
use crate::lint::rule::{Severity, SourceRule};

pub struct EmbeddedScriptIncludesCopyrightStatement;

declare_rule! {
    /// A Patched HTML page has an inline `<script>` whose first non-blank line
    /// carries a copyright statement: the word `copyright`, `(c)` followed by a
    /// word, or the copyright sign. Reported at pedantic severity with the
    /// extract (up to 20 characters before and 50 after the statement) as the
    /// extra, `extract of copyright statement: <extract>`, and the page as the
    /// Location.
    ///
    /// The inline scripts are those lintian's `cruft` Check walks: a script with
    /// a `src` attribute is skipped, and the walk stops at a `<script` with no
    /// closing `</script>`. Quilt's `.pc/` is skipped.
    ///
    /// Differs from lintian: the page is read whole, not in 16 KiB blocks.
    EmbeddedScriptIncludesCopyrightStatement,
    code = "LN1631",
    tag = "embedded-script-includes-copyright-statement",
    default_selected = false,
    experimental = false,
}

static HTML_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(?:x?html?\d?|xht)$").expect("valid regex"));

/// lintian's `.{0,20}((?:\bcopyright\b|[\(]c[\)]\s*\w|\N{COPYRIGHT SIGN}).{0,50})`
/// on the first line of a script; the group is the extract.
static COPYRIGHT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s).{0,20}((?:\bcopyright\b|\(c\)\s*\w|\x{00A9}).{0,50})").expect("valid regex")
});

impl SourceRule for EmbeddedScriptIncludesCopyrightStatement {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let texts = license_text::shared_text_files(ctx);
        let mut findings = Vec::new();
        for file in texts.iter() {
            let basename = file
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !HTML_NAME.is_match(&basename) {
                continue;
            }
            for script in license_text::inline_scripts(&file.lowercase) {
                let Some(first) = script.split('\n').find(|line| !line.trim().is_empty()) else {
                    continue;
                };
                let Some(captures) = COPYRIGHT.captures(first) else {
                    continue;
                };
                let extract = captures.get(1).map_or("", |group| group.as_str()).trim();
                findings.push((
                    file.path.clone(),
                    format!("extract of copyright statement: {extract}"),
                ));
            }
        }
        for (path, extra) in findings {
            ctx.diagnostic(
                Diagnostic::warn(extra)
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const PAGE: &str =
        "<html>\n<script>\n// Copyright (c) 2010 Example Corp\nvar a = 1;\n</script>\n</html>\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                // A script with a `src` is not read.
                (
                    "src.html",
                    TestFile::new("<script src=\"lib.js\">// copyright 2010</script>\n"),
                ),
                // No copyright statement in the first line.
                (
                    "plain.html",
                    TestFile::new("<script>\nvar a = 1;\n// copyright\n</script>\n"),
                ),
                // A `.js` file is not a page.
                ("app.js", TestFile::new("// Copyright (c) 2010 Example\n")),
                // Quilt's `.pc/` is not the packaging's file.
                (".pc/page.html", TestFile::new(PAGE)),
                // A script with no closing tag ends the walk.
                ("open.html", TestFile::new("<script>\n// copyright 2010\n")),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("page.html", TestFile::new(PAGE)),
            ],
            // The copyright sign counts, and so does the bare `copyright` word.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "sign.html",
                    TestFile::new("<script>\n\u{00a9} 2010 Example\n</script>\n"),
                ),
                (
                    "word.html",
                    TestFile::new("<script>\n\n  Copyright notice\n</script>\n"),
                ),
            ],
        ];
        SourceTreeTester::new(EmbeddedScriptIncludesCopyrightStatement, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::from(CONTROL),
        )])];
        let fail = vec![SourcePackageCase::from(vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("page.html", TestFile::new(PAGE)),
        ])];
        SourcePackageTester::new(EmbeddedScriptIncludesCopyrightStatement, pass, fail)
            .test_and_snapshot();
    }
}
