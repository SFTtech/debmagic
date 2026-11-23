use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep5_scan::{FormatUriHint, copyright_texts, format_uri_hint};

pub struct WikiCopyrightFormatUri;

declare_rule! {
    /// The copyright file's Format URI points at the DEP-5 wiki proposal.
    ///
    /// Reads `debian/copyright` and `debian/<installable>.copyright` (symlinks
    /// and files that are not UTF-8 are skipped). Any
    /// `wiki.debian.org/Proposals/CopyrightFormat` URI, with its `recall` or
    /// `rev` query, is a wiki draft and not a released format. Extra is the
    /// Format URI without its fragment; the Location is the copyright file.
    /// Pedantic, not selected by default.
    WikiCopyrightFormatUri,
    code = "LN1652",
    tag = "wiki-copyright-format-uri",
    default_selected = false,
    experimental = false,
}

impl SourceRule for WikiCopyrightFormatUri {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, text) in copyright_texts(ctx) {
            if let Some(FormatUriHint::Wiki(uri)) = format_uri_hint(&text) {
                ctx.diagnostic(
                    Diagnostic::warn(uri)
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
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn copyright(format: &str) -> TestFile {
        TestFile::new(format!(
            "Format: {format}\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![(
                "debian/copyright",
                copyright("https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/"),
            )],
            vec![(
                "debian/copyright",
                copyright("http://dep.debian.net/deps/dep5/"),
            )],
            vec![("debian/copyright", TestFile::new("Copyright 2024\n"))],
        ];
        let fail = vec![
            vec![(
                "debian/copyright",
                copyright("http://wiki.debian.org/Proposals/CopyrightFormat?recall=184"),
            )],
            vec![(
                "debian/copyright",
                copyright(
                    "http://wiki.debian.org/Proposals/CopyrightFormat?action=recall&rev=196#top",
                ),
            )],
        ];
        SourceTreeTester::new(WikiCopyrightFormatUri, pass, fail).test_and_snapshot();
    }
}
