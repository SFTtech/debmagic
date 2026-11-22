use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct SourceContainsPrebuiltDoxygenDocumentation;

declare_rule! {
    /// The source ships doxygen's generated documentation.
    ///
    /// Reports the directory (with a trailing slash) of a `doxygen.png` or `doxygen.sty` (any character
    /// in place of the dot), unless the source package is `doxygen` itself. Also
    /// reports an HTML file (`.html`, `.htm`, `.xhtml`, `.xht`, with an optional
    /// digit) that has `<meta name="generator" content="doxygen`, unless it
    /// uses a doxygen template variable (`$doxygenversion`, `$projectname`,
    /// `$projectnumber` or `$projectlogo`). Invalid UTF-8 counts as empty.
    /// Entries under `.pc/` are skipped. The directory (for the PNG and STY
    /// case) or the file is the Location; no extra. Reported at pedantic
    /// severity.
    SourceContainsPrebuiltDoxygenDocumentation,
    code = "LN1353",
    tag = "source-contains-prebuilt-doxygen-documentation",
    default_selected = false,
    experimental = false,
}

static DOXYGEN_ASSET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^doxygen.(?:png|sty)$").expect("valid regex"));
static HTML_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(?:x?html?\d?|xht)$").expect("valid regex"));
static DOXYGEN_GENERATOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<meta\s+name="generator"\s+content="doxygen"#).expect("valid regex")
});
static DOXYGEN_TEMPLATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$(?:doxygenversion|projectname|projectnumber|projectlogo)\b")
        .expect("valid regex")
});

impl SourceRule for SourceContainsPrebuiltDoxygenDocumentation {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let is_doxygen = ctx
            .debian_control()
            .and_then(|control| control.source().get("Source"))
            == Some("doxygen");
        let Some(files) = ctx.files() else {
            return;
        };
        let mut findings: Vec<(PathBuf, String)> = Vec::new();
        for entry in files.entries() {
            if !entry.is_file() || entry.path().starts_with(".pc") {
                continue;
            }
            let path = entry.path();
            let Some(basename) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if DOXYGEN_ASSET.is_match(basename) && !is_doxygen {
                let parent = path.parent().and_then(|dir| dir.to_str()).unwrap_or("");
                let directory =
                    PathBuf::from(format!("{}/", if parent.is_empty() { "." } else { parent }));
                findings.push((directory, String::new()));
            }
            if !HTML_NAME.is_match(basename) {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            let Ok(contents) = std::str::from_utf8(&bytes) else {
                continue;
            };
            if contents.is_empty() {
                continue;
            }
            let lowercase = contents.to_lowercase();
            if DOXYGEN_GENERATOR.is_match(&lowercase) && !DOXYGEN_TEMPLATE.is_match(&lowercase) {
                findings.push((path.to_path_buf(), String::new()));
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

    const CONTROL: &str = "Source: example\nMaintainer: Example <ex@example.com>\n\n";

    fn tree(files: Vec<(&str, &str)>) -> Vec<(String, TestFile)> {
        let mut tree = vec![("debian/control".to_string(), TestFile::new(CONTROL))];
        tree.extend(
            files
                .into_iter()
                .map(|(path, content)| (path.to_string(), TestFile::new(content))),
        );
        tree
    }

    const GENERATED: &str =
        "<html><head><meta name=\"generator\" content=\"Doxygen 1.9\"></head></html>\n";
    const TEMPLATE: &str = "<meta name=\"generator\" content=\"doxygen $doxygenversion\">\n";

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![("src/doc/index.html", TEMPLATE)]),
            tree(vec![("src/doc/index.html", "<html>plain</html>\n")]),
            tree(vec![("src/doc/index.txt", GENERATED)]),
            tree(vec![(".pc/fix/doc/index.html", GENERATED)]),
            tree(vec![("src/doc/index.html", "")]),
        ];
        let fail = vec![
            tree(vec![("src/doc/latex/doxygen.png", "png")]),
            tree(vec![("src/doc/html/index.html", GENERATED)]),
            tree(vec![("src/doc/html/index.xhtml", GENERATED)]),
        ];
        SourceTreeTester::new(SourceContainsPrebuiltDoxygenDocumentation, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(vec![(
            "src/doc/index.html",
            "<html>plain</html>\n",
        )]))];
        let fail = vec![SourcePackageCase::from(tree(vec![(
            "src/doc/index.html",
            GENERATED,
        )]))];
        SourcePackageTester::new(SourceContainsPrebuiltDoxygenDocumentation, pass, fail)
            .test_and_snapshot();
    }
}
