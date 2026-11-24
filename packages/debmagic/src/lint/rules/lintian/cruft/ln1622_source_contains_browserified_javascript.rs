use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_text::{self, TextFile};
use crate::lint::rule::{Severity, SourceRule};

pub struct SourceContainsBrowserifiedJavascript;

declare_rule! {
    /// A Patched JavaScript file, or an inline `<script>` of a Patched HTML
    /// page, is the output of a bundler: it matches one of lintian's
    /// browserify, webpack or browserify-lite patterns. Reported at pedantic
    /// severity with the code fragment that matched (`code fragment:` and the
    /// pattern's first group), or an empty extra when that group is absent;
    /// the Location is the file.
    ///
    /// A `.js` file is read as lintian's `cruft` Check reads it: its
    /// contiguous code (JSLint banner lines, comments, indentation and blank
    /// lines removed, lines joined) is matched. Minified names (`.min.js` and
    /// the like) are not read. The sphinx and pandoc search indexes, and the
    /// `srchidx.js`, `css_browser_selector(` and `jush.tr.` exceptions, are
    /// skipped, as lintian does before it looks for the pattern. Skipped too
    /// are the `.pc/` paths and the binary files. An HTML page is read
    /// through each inline `<script>` block, the ones with a `src` attribute
    /// excepted. Quilt's `debian/missing-sources/` is not skipped.
    ///
    /// Differs from lintian: a file whose license Tag already stopped
    /// lintian's check is still searched for bundler output.
    SourceContainsBrowserifiedJavascript,
    code = "LN1622",
    tag = "source-contains-browserified-javascript",
    default_selected = false,
    experimental = false,
}

static HTML_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(?:x?html?\d?|xht)$").expect("valid regex"));
static MINIFIED_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[-._](?:compiled|compressed|lite|min|pack(?:ed)?|prod|umd|yc)\.js$")
        .expect("valid regex")
});
static SETINDEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?ms)\A\s*search\.setindex\s*\s*\(\s*\{").expect("valid regex"));
static SEARCH_INDEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?ms)\A\s*var\s*search_index\s*=").expect("valid regex"));
static PROFILES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?ms)\A\s*profiles\s*=\s*new\s*Array\s*\(").expect("valid regex")
});

/// The JavaScript files lintian's Check gives up on before the browserify
/// table, the sphinx and pandoc indexes among them (those Tags are other
/// Rules).
fn is_exempt_script(basename: &str, lowercase: &str) -> bool {
    if basename == "searchindex.js" && SETINDEX.is_match(lowercase) {
        return true;
    }
    if basename == "search_index.js" {
        return SEARCH_INDEX.is_match(lowercase);
    }
    if basename == "srchidx.js" {
        return PROFILES.is_match(lowercase);
    }
    lowercase.contains("css_browser_selector(") || lowercase.contains("jush.tr.")
}

fn basename(file: &TextFile) -> String {
    file.path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl SourceRule for SourceContainsBrowserifiedJavascript {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let texts = license_text::shared_text_files(ctx);
        let mut findings = Vec::new();
        for file in texts.iter() {
            let name = basename(file);
            if HTML_NAME.is_match(&name) {
                for script in license_text::inline_scripts(&file.lowercase) {
                    let contiguous = license_text::contiguous_code(script);
                    if let Some(extra) = license_text::browserify_extra(&contiguous) {
                        findings.push((file.path.clone(), extra));
                    }
                }
                continue;
            }
            let path = file.path.to_string_lossy();
            if !path.ends_with(".js")
                || MINIFIED_NAME.is_match(&path)
                || is_exempt_script(&name, &file.lowercase)
            {
                continue;
            }
            let contiguous = license_text::contiguous_code(&file.lowercase);
            if let Some(extra) = license_text::browserify_extra(&contiguous) {
                findings.push((file.path.clone(), extra));
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

    const LITE: &str = "(function(modules, cache, entry) {\n  return modules;\n})({});\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("src/app.js", TestFile::new("var a = 1;\n")),
                // Minified by name: not read.
                ("src/app.min.js", TestFile::new(LITE)),
                // Quilt's `.pc/` is not the packaging's file.
                (".pc/app.js", TestFile::new(LITE)),
                // Not a script.
                ("src/notes.txt", TestFile::new(LITE)),
                // Lintian strips the comment before it matches.
                (
                    "src/commented.js",
                    TestFile::new(
                        "/* (function(modules, cache, entry) { return 1; }) */\nvar a;\n",
                    ),
                ),
                // A sphinx search index is its own Tag.
                (
                    "doc/searchindex.js",
                    TestFile::new(format!("search.setindex({{ }});\n{LITE}")),
                ),
                // A script with a `src` is not read.
                (
                    "page.html",
                    TestFile::new(format!("<script src=\"lib.js\">{LITE}</script>")),
                ),
                // A binary file is not read.
                ("src/bin.js", TestFile::new(format!("\0{LITE}"))),
                // Lintian's exception for css_browser_selector.
                (
                    "src/css.js",
                    TestFile::new(format!("css_browser_selector(\n{LITE}")),
                ),
                ("src/hardlink.js", TestFile::new(LITE).hardlink()),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("src/app.js", TestFile::new(LITE)),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "src/webpack.js",
                    TestFile::new(
                        "(function webpackUniversalModuleDefinition(root, factory) {\n  return 1;\n})\n",
                    ),
                ),
                (
                    "src/browserify.js",
                    TestFile::new(
                        "if (\"object\" == typeof exports && \"undefined\" != typeof module) {}\n",
                    ),
                ),
            ],
            // An inline script of an HTML page.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "page.html",
                    TestFile::new(format!("<html><script>\n{LITE}</script></html>\n")),
                ),
            ],
            // A search index that is not the sphinx form is still searched.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "doc/searchindex.js",
                    TestFile::new(format!("var x;\n{LITE}")),
                ),
            ],
            // A file of `debian/missing-sources/` is not skipped by this Check.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("debian/missing-sources/app.js", TestFile::new(LITE)),
            ],
        ];
        SourceTreeTester::new(SourceContainsBrowserifiedJavascript, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::from(CONTROL),
        )])];
        let fail = vec![SourcePackageCase::from(vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("src/app.js", TestFile::new(LITE)),
        ])];
        SourcePackageTester::new(SourceContainsBrowserifiedJavascript, pass, fail)
            .test_and_snapshot();
    }
}
