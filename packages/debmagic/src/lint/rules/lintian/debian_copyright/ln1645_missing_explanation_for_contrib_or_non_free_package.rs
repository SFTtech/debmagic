use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_copyright::DebianCopyright;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep5_scan::copyright_texts;

pub struct MissingExplanationForContribOrNonFreePackage;

declare_rule! {
    /// A contrib or non-free source package has no explanation in its copyright
    /// header.
    ///
    /// The source paragraph's `Section` is `contrib` or `non-free` (or below
    /// them), and the header paragraph of a machine-readable copyright file
    /// declares neither `Comment` nor `Disclaimer`. Extra is empty; the Location
    /// is the header's first line. Info severity, not selected by default.
    /// Skipped when the copyright file is not machine-readable or has a syntax
    /// error, or when `debian/control` does not parse.
    MissingExplanationForContribOrNonFreePackage,
    code = "LN1645",
    tag = "missing-explanation-for-contrib-or-non-free-package",
    default_selected = false,
    experimental = false,
}

static CONTRIB_OR_NON_FREE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:contrib|non-free)(?:/.+)?$").expect("valid regex"));

impl SourceRule for MissingExplanationForContribOrNonFreePackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(section) = ctx
            .debian_control()
            .and_then(|control| control.source().get("Section"))
            .map(|value| value.trim().to_string())
        else {
            return;
        };
        if !CONTRIB_OR_NON_FREE.is_match(&section) {
            return;
        }
        for (path, text) in copyright_texts(ctx) {
            let Some(header) = DebianCopyright::parse(&text)
                .paragraphs()
                .and_then(|all| all.first().cloned())
            else {
                continue;
            };
            if header.declares("Comment") || header.declares("Disclaimer") {
                continue;
            }
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: Some(header.start_line()),
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const FORMAT: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";

    fn copyright(header_extra: &str) -> TestFile {
        TestFile::new(format!(
            "{FORMAT}{header_extra}\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n"
        ))
    }

    fn control(section: &str) -> TestFile {
        TestFile::new(format!(
            "Source: example\nSection: {section}\n\nPackage: example\nArchitecture: all\nDescription: x\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![
                ("debian/control", control("misc")),
                ("debian/copyright", copyright("")),
            ],
            vec![
                ("debian/control", control("non-free/libs")),
                ("debian/copyright", copyright("Comment: why\n")),
            ],
            vec![
                ("debian/control", control("contrib")),
                ("debian/copyright", copyright("Disclaimer: why\n")),
            ],
            vec![
                ("debian/control", control("non-freeish")),
                ("debian/copyright", copyright("")),
            ],
            vec![
                ("debian/control", control("non-free")),
                ("debian/copyright", TestFile::new("Copyright 2024\n")),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", control("contrib")),
                ("debian/copyright", copyright("")),
            ],
            vec![
                ("debian/control", control("non-free")),
                ("debian/copyright", copyright("Upstream-Name: x\n")),
            ],
        ];
        SourceTreeTester::new(MissingExplanationForContribOrNonFreePackage, pass, fail)
            .test_and_snapshot();
    }
}
