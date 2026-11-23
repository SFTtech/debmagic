use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep5_scan::{FormatUriHint, copyright_texts, format_uri_hint};

pub struct UnknownCopyrightFormatUri;

declare_rule! {
    /// The machine-readable copyright file has a Format that lintian does not
    /// know.
    ///
    /// Reads `debian/copyright` and `debian/<installable>.copyright` (symlinks
    /// and files that are not UTF-8 are skipped). Reported when the first
    /// paragraph's `Format` field is missing or is not a single word, with an
    /// empty extra, or when its URI is not a known copyright-format URI, with
    /// the URI as extra. Boilerplate URIs have their own Tag. The Location is
    /// the copyright file. Pedantic, not selected by default.
    UnknownCopyrightFormatUri,
    code = "LN1650",
    tag = "unknown-copyright-format-uri",
    default_selected = false,
    experimental = false,
}

impl SourceRule for UnknownCopyrightFormatUri {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, text) in copyright_texts(ctx) {
            let Some(FormatUriHint::Unknown(uri)) = format_uri_hint(&text) else {
                continue;
            };
            ctx.diagnostic(
                Diagnostic::warn(uri.unwrap_or_default())
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
            // Boilerplate and wiki URIs have their own Tags.
            vec![("debian/copyright", copyright("<VERSIONED_FORMAT_URL>"))],
            vec![(
                "debian/copyright",
                copyright("http://wiki.debian.org/Proposals/CopyrightFormat?recall=184"),
            )],
            // Not machine-readable: no Format marker at all.
            vec![("debian/copyright", TestFile::new("Copyright 2024\n"))],
        ];
        let fail = vec![
            vec![("debian/copyright", copyright("https://example.org/dep5/"))],
            // lintian strips only the `#` of a fragment, so the text stays.
            vec![(
                "debian/copyright",
                copyright(
                    "https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/#frag",
                ),
            )],
            // A Format line that is not a single word has no URI.
            vec![(
                "debian/copyright",
                copyright(
                    "https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/ extra",
                ),
            )],
        ];
        SourceTreeTester::new(UnknownCopyrightFormatUri, pass, fail).test_and_snapshot();
    }
}
