use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_text;
use crate::lint::rule::{Severity, SourceRule};

pub struct SourceContainsPrebuiltPandocDocumentation;

declare_rule! {
    /// A Patched file is the `search_index.js` that Pandoc writes beside its
    /// HTML documentation: its text starts `var search_index =`. Reported at
    /// pedantic severity with an empty extra and the directory that holds the
    /// file as the Location (with a trailing slash), the pointer form lintian's
    /// `cruft` Check emits.
    ///
    /// Quilt's `.pc/` is skipped. The file is read whole, as lintian reads its
    /// first block. Not skipped in a non-free package: the Check does not look
    /// at the licence there.
    SourceContainsPrebuiltPandocDocumentation,
    code = "LN1627",
    tag = "source-contains-prebuilt-pandoc-documentation",
    default_selected = false,
    experimental = false,
}

static SEARCH_INDEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?ms)\A\s*var\s*search_index\s*=").expect("valid regex"));

/// The directory of `path` with a trailing slash, as lintian's
/// `parent_dir->pointer` prints it; the top level is `./`.
pub(crate) fn directory_pointer(path: &Path) -> PathBuf {
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => PathBuf::from(format!("{}/", dir.display())),
        _ => PathBuf::from("./"),
    }
}

impl SourceRule for SourceContainsPrebuiltPandocDocumentation {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let flagged: Vec<_> = license_text::text_files(files)
            .into_iter()
            .filter(|file| {
                file.path
                    .file_name()
                    .is_some_and(|name| name == "search_index.js")
                    && SEARCH_INDEX.is_match(&file.lowercase)
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

    const PANDOC: &str = "var search_index = {\"docs\": []};\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                // Another index, not Pandoc's.
                ("docs/search_index.js", TestFile::new("var other = 1;\n")),
                // A minified copy is not read.
                ("docs/search_index.min.js", TestFile::new(PANDOC)),
                // Quilt's `.pc/` is not the packaging's file.
                (".pc/docs/search_index.js", TestFile::new(PANDOC)),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "oldfalsepositives/pandoc/search_index.js",
                    TestFile::new(PANDOC),
                ),
            ],
            // At the top level, the directory is the root.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("search_index.js", TestFile::new(PANDOC)),
            ],
        ];
        SourceTreeTester::new(SourceContainsPrebuiltPandocDocumentation, pass, fail)
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
            ("docs/search_index.js", TestFile::new(PANDOC)),
        ])];
        SourcePackageTester::new(SourceContainsPrebuiltPandocDocumentation, pass, fail)
            .test_and_snapshot();
    }
}
