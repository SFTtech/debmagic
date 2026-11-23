use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep5_scan::{FormatUriHint, copyright_texts, format_uri_hint};

pub struct OutOfDateCopyrightFormatUri;

declare_rule! {
    /// The copyright file's Format URI names a DEP-5 draft revision older than
    /// the last significant change.
    ///
    /// Reads `debian/copyright` and `debian/<installable>.copyright` (symlinks
    /// and files that are not UTF-8 are skipped). A `dep` or `svn.debian.org`
    /// URI with a `rev=N` below 166 is out-of-date. Wiki and unversioned URIs
    /// have their own Tags, and a copyright-format release URI is never
    /// out-of-date. Extra is the Format URI without its fragment; the Location
    /// is the copyright file. Pedantic, not selected by default.
    OutOfDateCopyrightFormatUri,
    code = "LN1647",
    tag = "out-of-date-copyright-format-uri",
    default_selected = false,
    experimental = false,
}

impl SourceRule for OutOfDateCopyrightFormatUri {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, text) in copyright_texts(ctx) {
            if let Some(FormatUriHint::OutOfDate(uri)) = format_uri_hint(&text) {
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
                copyright("http://svn.debian.org/wsvn/dep/web/deps/dep5.mdwn?op=file&rev=166"),
            )],
            vec![(
                "debian/copyright",
                copyright("http://svn.debian.org/wsvn/dep/web/deps/dep5.mdwn?op=file&rev=300"),
            )],
            // Unversioned and wiki URIs have their own Tags.
            vec![(
                "debian/copyright",
                copyright("http://dep.debian.net/deps/dep5/"),
            )],
            vec![(
                "debian/copyright",
                copyright("http://wiki.debian.org/Proposals/CopyrightFormat?recall=184"),
            )],
            // Not machine-readable.
            vec![("debian/copyright", TestFile::new("Copyright 2024\n"))],
        ];
        let fail = vec![
            vec![(
                "debian/copyright",
                copyright("http://svn.debian.org/wsvn/dep/web/deps/dep5.mdwn?op=file&rev=135"),
            )],
            vec![(
                "debian/copyright",
                copyright("https://svn.debian.org/viewvc/dep/web/deps/dep5.mdwn?revision=10"),
            )],
        ];
        SourceTreeTester::new(OutOfDateCopyrightFormatUri, pass, fail).test_and_snapshot();
    }
}
