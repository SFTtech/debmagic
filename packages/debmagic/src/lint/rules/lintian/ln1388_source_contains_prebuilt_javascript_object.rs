use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::source_missing::{is_prebuilt_javascript, visited_files};

pub struct SourceContainsPrebuiltJavascriptObject;

declare_rule! {
    /// The Source tree or Source package contains a JavaScript file that is
    /// built, not written: a minified, packed or compiled one.
    ///
    /// A file is reported when its name ends in `.compiled.js`,
    /// `.compressed.js`, `.lite.js`, `.min.js`, `.pack.js`, `.packed.js`,
    /// `.prod.js`, `.umd.js` or `.yc.js` (the separator may also be `-` or
    /// `_`; case is ignored), or when a file named `*.js` has a line longer
    /// than 512 bytes. The Oracle `deployJava.js` is exempt. Reported at
    /// pedantic severity with an empty extra and the path as the Location: in
    /// lintian 2.144.0 the Check emits it only as a pointer.
    ///
    /// Walks the packaging File tree (Source tree, or Source package Patched
    /// files). Quilt `.pc/` is filtered out and `debian/missing-sources/` is
    /// skipped. The sibling `source-is-missing` (LN1390) reports those of the
    /// files with no source in the tree.
    SourceContainsPrebuiltJavascriptObject,
    code = "LN1388",
    tag = "source-contains-prebuilt-javascript-object",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsPrebuiltJavascriptObject {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let paths: Vec<_> = visited_files(files)
            .filter(|entry| is_prebuilt_javascript(files, entry))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
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

    fn long_line() -> String {
        format!("var a = '{}';\n", "x".repeat(600))
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("js/app.js", TestFile::new("var a = 1;\n")),
                ("js/minimal.js", TestFile::new("var a = 1;\n")),
                // The name says nothing, and the line is not long enough.
                ("js/app-lite.json", TestFile::new("{}\n")),
                (
                    "js/admin.js",
                    TestFile::new(format!("{}\n", "x".repeat(512))),
                ),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (".pc/app.min.js", TestFile::new("ignored")),
                (
                    "debian/missing-sources/app.min.js",
                    TestFile::new("sourced"),
                ),
            ],
            // A long line in a page is not this Tag.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("page.html", TestFile::new(long_line())),
            ],
            // The Oracle's deployJava library.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "deployJava.js",
                    TestFile::new(format!(
                        "var deployJava = function() {{}};\n{}",
                        long_line()
                    )),
                ),
            ],
            // A directory named like a minified file is not a file.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("lib.min.js", TestFile::directory()),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("build/foo.min.js", TestFile::new("var a=1;")),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("a/test_min.js", TestFile::new("")),
                ("a/test-yc.js", TestFile::new("")),
                ("a/test.compressed.js", TestFile::new("")),
                ("a/TEST.PACKED.JS", TestFile::new("")),
            ],
            // A long line in a script, whatever the name.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("js/search_index.js", TestFile::new(long_line())),
            ],
            // A hardlink is a file too.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("lib.min.js", TestFile::new("a").hardlink()),
            ],
            // A file named deployJava.js that is not the library.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("deployJava.js", TestFile::new(long_line())),
            ],
        ];
        SourceTreeTester::new(SourceContainsPrebuiltJavascriptObject, pass, fail)
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
            ("js/app.min.js", TestFile::new("var a=1;")),
        ])];
        SourcePackageTester::new(SourceContainsPrebuiltJavascriptObject, pass, fail)
            .test_and_snapshot();
    }
}
