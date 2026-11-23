use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_text;
use crate::lint::rule::{Severity, SourceRule};

use super::ln1627_source_contains_prebuilt_pandoc_documentation::directory_pointer;

pub struct SourceContainsPrebuiltSphinxDocumentation;

declare_rule! {
    /// A Patched file is the `searchindex.js` that Sphinx writes beside its
    /// HTML documentation: its text starts `Search.setIndex({`. Reported at
    /// pedantic severity with an empty extra and the directory that holds the
    /// file as the Location (with a trailing slash), the pointer form lintian's
    /// `cruft` Check emits.
    ///
    /// Quilt's `.pc/` is skipped. The file is read whole, as lintian reads its
    /// first block. Not skipped in a non-free package: the Check does not look
    /// at the licence there.
    SourceContainsPrebuiltSphinxDocumentation,
    code = "LN1628",
    tag = "source-contains-prebuilt-sphinx-documentation",
    default_selected = false,
    experimental = false,
}

static SET_INDEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?ms)\A\s*search\.setindex\s*\s*\(\s*\{").expect("valid regex"));

impl SourceRule for SourceContainsPrebuiltSphinxDocumentation {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let flagged: Vec<_> = license_text::text_files(files)
            .into_iter()
            .filter(|file| {
                file.path
                    .file_name()
                    .is_some_and(|name| name == "searchindex.js")
                    && SET_INDEX.is_match(&file.lowercase)
            })
            .map(|file| directory_pointer(&file.path))
            .collect();
        for path in flagged {
            ctx.diagnostic(
                Diagnostic::warn("")
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

    const SPHINX: &str = "Search.setIndex({\"docnames\": []});\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                // Sphinx writes the name with a `setIndex` call only.
                ("docs/searchindex.js", TestFile::new("var x = 1;\n")),
                // A minified copy is not read.
                ("docs/searchindex.min.js", TestFile::new(SPHINX)),
                // Quilt's `.pc/` is not the packaging's file.
                (".pc/docs/searchindex.js", TestFile::new(SPHINX)),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "src/sphinx-1.22/_build/html/searchindex.js",
                    TestFile::new(SPHINX),
                ),
            ],
            // A search index at the top level sits in the root.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("searchindex.js", TestFile::new(SPHINX)),
            ],
        ];
        SourceTreeTester::new(SourceContainsPrebuiltSphinxDocumentation, pass, fail)
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
            ("docs/searchindex.js", TestFile::new(SPHINX)),
        ])];
        SourcePackageTester::new(SourceContainsPrebuiltSphinxDocumentation, pass, fail)
            .test_and_snapshot();
    }
}
