use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::content::{self, Opening};

pub struct BadSoLinkWithinManualPage;

declare_rule! {
    /// A short manual page includes another page with a bad `.so` path.
    ///
    /// A regular file or hardlink in `usr/share/man/.../manN/` whose stored
    /// size is under 256 bytes. Gzip magic `1f 8b` is decompressed first.
    /// Leading `.\"` comments are skipped while the next line is Perl-true.
    /// The line must match `.so` plus whitespace. `manN/rest` is accepted.
    /// `lang/manN/name.digit` with an optional non-space tail and optional
    /// `.gz` is accepted. Anything else, including a missing destination, is
    /// flagged. A larger file is not judged here. Udebs are skipped. Extra
    /// is empty; the Data files path is the Location.
    BadSoLinkWithinManualPage,
    code = "LN0273",
    tag = "bad-so-link-within-manual-page",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BadSoLinkWithinManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        for page in content::pages(files) {
            if !matches!(page.opening, Opening::So { bad: true }) {
                continue;
            }
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: page.path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn text(path: &str, body: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::new(body))
    }

    fn gzip(path: &str, body: &str) -> (String, TestFile) {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(body.as_bytes()).expect("gzip");
        (
            path.to_string(),
            TestFile::from_bytes(encoder.finish().expect("gzip")),
        )
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    fn padded_bad_so() -> String {
        let mut text = ".so nowhere\n".to_string();
        text.push_str(&"x".repeat(256));
        text
    }

    #[test]
    fn test() {
        let pass = vec![
            case(vec![]),
            case(vec![
                text("usr/share/man/man1/ok.1", ".so man1/foo.1.gz\n"),
                text("usr/share/man/man1/bare.1", ".so man1/foo\n"),
                text("usr/share/man/man3/sec.3", ".so man3/bar.3\n"),
                text("usr/share/man/de/man1/lang.1", ".so de/man1/foo.1.gz\n"),
                text(
                    "usr/share/man/man1/comment.1",
                    ".\\\" comment\n.so man1/foo.1\n",
                ),
                text(
                    "usr/share/man/man1/body.1",
                    ".TH FOO 1\n.SH NAME\nfoo \\- bar\n",
                ),
            ]),
            // At 256 bytes the `.so` check does not run.
            case(vec![text("usr/share/man/man1/big.1", &padded_bad_so())]),
            case(vec![(
                "usr/share/man/man1/bad.1".to_string(),
                TestFile::symlink("../somewhere/invalid/manpage.1"),
            )]),
            case(vec![text(
                "usr/share/man/man1/bad.1",
                ".so ../somewhere/invalid/manpage.1\n",
            )])
            .udeb(),
        ];
        let fail = vec![case(vec![
            text(
                "usr/share/man/man1/bad-so.1",
                ".so ../somewhere/invalid/manpage.1\n",
            ),
            text("usr/share/man/man1/noslash.1", ".so foo\n"),
            text("usr/share/man/man1/lang.1", ".so de/foo.1\n"),
            text("usr/share/man/man1/rest.1", ".so de/man1/not-a-page\n"),
            text("usr/share/man/man1/blank.1", ".so \n"),
            text("usr/share/man/man1/comment.1", ".\\\" note\n.so ../bad.1\n"),
            (
                "usr/share/man/man1/hard.1".to_string(),
                TestFile::new(".so ../bad.1\n").hardlink(),
            ),
            gzip(
                "usr/share/man/man1/bad.1.gz",
                ".so ../somewhere/invalid/manpage.1\n",
            ),
        ])];
        BinaryPackageTester::new(BadSoLinkWithinManualPage, pass, fail).test_and_snapshot();
    }
}
