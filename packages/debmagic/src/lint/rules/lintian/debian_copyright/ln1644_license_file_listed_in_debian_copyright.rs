use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_copyright::DebianCopyright;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep5_scan::{copyright_texts, wildcards};

pub struct LicenseFileListedInDebianCopyright;

declare_rule! {
    /// A license notice file that is shipped is also listed as a `Files` pattern
    /// of the machine-readable copyright.
    ///
    /// A shipped `COPYING*` or `LICENSE` file (at any depth) whose name is also
    /// a `Files` pattern of a paragraph after the header. Such a file does not
    /// need its own paragraph. Extra is the file's name; the Location is the
    /// copyright file. Info severity, not selected by default. Skipped when the
    /// file is not machine-readable or has a syntax error, as lintian's Dep5
    /// flow stops there.
    LicenseFileListedInDebianCopyright,
    code = "LN1644",
    tag = "license-file-listed-in-debian-copyright",
    default_selected = false,
    experimental = false,
}

static NOTICE_FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|/)(COPYING[^/]*|LICENSE)$").expect("valid regex"));

impl SourceRule for LicenseFileListedInDebianCopyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(shipped) = ctx.shipped_files() else {
            return;
        };
        let notices: Vec<String> = shipped
            .file_names()
            .iter()
            .filter(|name| NOTICE_FILE.is_match(name))
            .cloned()
            .collect();
        if notices.is_empty() {
            return;
        }
        for (path, text) in copyright_texts(ctx) {
            let Some(all) = DebianCopyright::parse(&text)
                .paragraphs()
                .map(<[_]>::to_vec)
            else {
                continue;
            };
            let Some(followers) = all.get(1..) else {
                continue;
            };
            let listed = wildcards(followers, shipped.file_names()).sections_by_wildcard;
            for notice in &notices {
                if listed.contains_key(notice) {
                    ctx.diagnostic(
                        Diagnostic::warn(notice.clone())
                            .with_severity(Severity::Info)
                            .with_location(Location {
                                path: path.clone(),
                                line: None,
                                column: None,
                            }),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const FORMAT: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";

    fn copyright(pattern: &str) -> TestFile {
        TestFile::new(format!(
            "{FORMAT}\nFiles: {pattern}\nCopyright: 2024 Someone\nLicense: MIT\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![
                ("COPYING", TestFile::new("text")),
                ("debian/copyright", copyright("src/*")),
            ],
            // A notice file that is not shipped is not listed.
            vec![("debian/copyright", copyright("COPYING"))],
            // Not machine-readable: the Dep5 flow never reaches the listing.
            vec![
                ("COPYING", TestFile::new("text")),
                ("debian/copyright", TestFile::new("Copyright 2024\n")),
            ],
        ];
        let fail = vec![
            vec![
                ("COPYING", TestFile::new("text")),
                ("debian/copyright", copyright("COPYING")),
            ],
            vec![
                ("docs/LICENSE", TestFile::new("text")),
                ("debian/copyright", copyright("docs/LICENSE")),
            ],
        ];
        SourceTreeTester::new(LicenseFileListedInDebianCopyright, pass, fail).test_and_snapshot();
    }
}
