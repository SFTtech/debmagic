use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_copyright::{DebianCopyright, format_uri};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep5_scan::copyright_texts;

pub struct InsecureCopyrightFormatUri;

declare_rule! {
    /// The machine-readable copyright's Format URI is a plain `http://` link to
    /// a released copyright-format document on `www.debian.org`.
    ///
    /// Reads `debian/copyright` and `debian/<installable>.copyright` (symlinks
    /// and files that are not UTF-8 are skipped). Only a file that lintian
    /// reads as machine-readable reaches the check, so a wiki, Subversion or
    /// boilerplate URI, and an unknown or outdated format, report nothing.
    /// Extra is the Format URI as lintian extracts it; the Location is the
    /// copyright file. Pedantic, selected only when asked for.
    InsecureCopyrightFormatUri,
    code = "LN1321",
    tag = "insecure-copyright-format-uri",
    default_selected = false,
    experimental = false,
}

impl SourceRule for InsecureCopyrightFormatUri {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, text) in copyright_texts(ctx) {
            // lintian's Dep5 flow reaches the insecure branch only for a
            // machine-readable file whose Format URI names a release document.
            if !matches!(
                DebianCopyright::parse(&text),
                DebianCopyright::MachineReadable { .. }
            ) {
                continue;
            }
            let Some(uri) = format_uri(&text) else {
                continue;
            };
            if !uri.starts_with("http://www.debian.org/") {
                continue;
            }
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

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn copyright(format: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/copyright",
            TestFile::new(format!(
                "Format: {format}\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n"
            )),
        )]
    }

    #[test]
    fn source() {
        let pass = vec![
            copyright("https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/"),
            copyright("https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/#frag"),
            copyright("http://example.org/doc/packaging-manuals/copyright-format/1.0/"),
            copyright(
                "http://www.debian.org/doc/packaging-manuals/copyright-format/1.0/rev=REVISION",
            ),
            copyright("https://wiki.debian.org/Proposals/CopyrightFormat"),
            copyright("http://dep.debian.net/deps/dep5/"),
            // lintian strips only the '#' (`s/^([^#\s]+)#/$1/`), so the
            // fragment text stays in the URI and no release version matches.
            copyright("http://www.debian.org/doc/packaging-manuals/copyright-format/1.0/#frag"),
        ];
        let fail = vec![
            copyright("http://www.debian.org/doc/packaging-manuals/copyright-format/1.0/"),
            copyright("http://www.debian.org/doc/copyright-format/1.0"),
            vec![
                (
                    "debian/control",
                    TestFile::new(
                        "Source: example\nMaintainer: Example <ex@example.com>\nStandards-Version: 4.7.2\n\nPackage: bar\nArchitecture: all\nDescription: example\n extra\n",
                    ),
                ),
                (
                    "debian/copyright",
                    TestFile::new(
                        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n",
                    ),
                ),
                (
                    "debian/bar.copyright",
                    TestFile::new(
                        "Format: http://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n",
                    ),
                ),
            ],
        ];
        SourceTreeTester::new(InsecureCopyrightFormatUri, pass, fail).test_and_snapshot();
    }
}
