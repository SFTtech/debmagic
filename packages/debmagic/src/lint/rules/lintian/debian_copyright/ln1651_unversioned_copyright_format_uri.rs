use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep5_scan::{FormatUriHint, copyright_texts, format_uri_hint};

pub struct UnversionedCopyrightFormatUri;

declare_rule! {
    /// The copyright file's Format URI is an unversioned DEP-5 draft URI.
    ///
    /// Reads `debian/copyright` and `debian/<installable>.copyright` (symlinks
    /// and files that are not UTF-8 are skipped). The `dep.debian.net/deps/dep5`
    /// URI, and a `svn.debian.org` DEP-5 URI without a revision, are `0+svn`
    /// versions with no revision; the URI is extra. Revisioned URIs are handled
    /// by the out-of-date Rule. The Location is the copyright file. Pedantic, not
    /// selected by default.
    UnversionedCopyrightFormatUri,
    code = "LN1651",
    tag = "unversioned-copyright-format-uri",
    default_selected = false,
    experimental = false,
}

impl SourceRule for UnversionedCopyrightFormatUri {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, text) in copyright_texts(ctx) {
            if let Some(FormatUriHint::Unversioned(uri)) = format_uri_hint(&text) {
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
                copyright("http://svn.debian.org/wsvn/dep/web/deps/dep5.mdwn?op=file&rev=300"),
            )],
            vec![(
                "debian/copyright",
                copyright("http://wiki.debian.org/Proposals/CopyrightFormat?recall=184"),
            )],
            vec![("debian/copyright", TestFile::new("Copyright 2024\n"))],
        ];
        let fail = vec![
            vec![(
                "debian/copyright",
                copyright("http://dep.debian.net/deps/dep5/"),
            )],
            vec![(
                "debian/copyright",
                copyright("https://dep-team.pages.debian.net/deps/dep5"),
            )],
            vec![(
                "debian/copyright",
                copyright("http://svn.debian.org/wsvn/dep/web/deps/dep5.mdwn"),
            )],
        ];
        SourceTreeTester::new(UnversionedCopyrightFormatUri, pass, fail).test_and_snapshot();
    }
}
