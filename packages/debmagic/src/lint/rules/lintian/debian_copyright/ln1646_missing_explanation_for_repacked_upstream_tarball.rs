use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_copyright::DebianCopyright;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep5_scan::{copyright_texts, repacked};

pub struct MissingExplanationForRepackedUpstreamTarball;

declare_rule! {
    /// A repacked upstream tarball from a URL has no explanation in the copyright
    /// header.
    ///
    /// The Source package's upstream version is repacked (`dfsg`, `debian`,
    /// `ds` or `repack` in the changelog version, lintian's `repacked`), and the
    /// header paragraph of a machine-readable copyright file has a `Source` that
    /// starts with `http://` or `https://`, but declares neither `Comment` nor
    /// `Files-Excluded`. Extra is empty; the Location is the header's first line.
    /// Info severity, not selected by default. Skipped when the copyright file
    /// is not machine-readable or has a syntax error.
    MissingExplanationForRepackedUpstreamTarball,
    code = "LN1646",
    tag = "missing-explanation-for-repacked-upstream-tarball",
    default_selected = false,
    experimental = false,
}

impl SourceRule for MissingExplanationForRepackedUpstreamTarball {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if !repacked(ctx) {
            return;
        }
        for (path, text) in copyright_texts(ctx) {
            let Some(header) = DebianCopyright::parse(&text)
                .paragraphs()
                .and_then(|all| all.first().cloned())
            else {
                continue;
            };
            let source = header.value("Source");
            if !(source.starts_with("http://") || source.starts_with("https://")) {
                continue;
            }
            if header.declares("Comment") || header.declares("Files-Excluded") {
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
            "{FORMAT}Source: https://example.org/\n{header_extra}\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n"
        ))
    }

    fn changelog(version: &str) -> TestFile {
        TestFile::new(format!(
            "example ({version}) unstable; urgency=medium\n\n  * Release.\n\n -- Someone <someone@example.org>  Mon, 01 Jan 2024 00:00:00 +0000\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            // Not repacked.
            vec![
                ("debian/changelog", changelog("1.0-1")),
                ("debian/copyright", copyright("")),
            ],
            vec![
                ("debian/changelog", changelog("1.0+dfsg-1")),
                ("debian/copyright", copyright("Comment: why\n")),
            ],
            vec![
                ("debian/changelog", changelog("1.0+ds-1")),
                ("debian/copyright", copyright("Files-Excluded: x\n")),
            ],
            // Not a URL in the Source field.
            vec![
                ("debian/changelog", changelog("1.0+dfsg-1")),
                (
                    "debian/copyright",
                    TestFile::new(format!(
                        "{FORMAT}Source: upstream\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n"
                    )),
                ),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/changelog", changelog("1.0+dfsg-1")),
                ("debian/copyright", copyright("")),
            ],
            vec![
                ("debian/changelog", changelog("1.0~repack1-1")),
                ("debian/copyright", copyright("Upstream-Name: x\n")),
            ],
        ];
        SourceTreeTester::new(MissingExplanationForRepackedUpstreamTarball, pass, fail)
            .test_and_snapshot();
    }
}
