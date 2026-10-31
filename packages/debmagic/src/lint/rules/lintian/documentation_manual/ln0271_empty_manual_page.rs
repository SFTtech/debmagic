use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::content::{self, Opening};

pub struct EmptyManualPage;

declare_rule! {
    /// The manual page has no body.
    ///
    /// A regular file or hardlink in `usr/share/man/.../manN/` whose stored
    /// size is under 256 bytes. Gzip magic `1f 8b` is decompressed first.
    /// Leading `.\"` comments are skipped while the next line is still
    /// present and Perl-true (`"0"` is false). The page is empty when that
    /// line is missing or false. A larger file is not judged here. Udebs are
    /// skipped. Extra is empty; the Data files path is the Location.
    EmptyManualPage,
    code = "LN0271",
    tag = "empty-manual-page",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for EmptyManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        for page in content::pages(files) {
            if !matches!(page.opening, Opening::Empty) {
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

    fn gzip_empty(path: &str) -> (String, TestFile) {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"").expect("gzip");
        (
            path.to_string(),
            TestFile::from_bytes(encoder.finish().expect("gzip")),
        )
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn test() {
        let pass = vec![
            case(vec![]),
            case(vec![text(
                "usr/share/man/man1/foo.1",
                ".TH FOO 1\n.SH NAME\nfoo \\- bar\n",
            )]),
            case(vec![text(
                "usr/share/man/man1/comment.1",
                ".\\\" only a comment\n",
            )]),
            case(vec![text("usr/share/man/man1/spaces.1", "   \n")]),
            case(vec![text("usr/share/man/man1/link.1", ".so man1/foo.1\n")]),
            // 256 newlines are at the size gate, so the empty opening is ignored.
            case(vec![text("usr/share/man/man1/long.1", &"\n".repeat(256))]),
            case(vec![text("usr/share/doc/empty", "")]),
            case(vec![(
                "usr/share/man/man1/empty.1".to_string(),
                TestFile::symlink("missing"),
            )]),
            case(vec![text("usr/share/man/man1/empty.1", "")]).udeb(),
        ];
        let fail = vec![case(vec![
            text("usr/share/man/man1/empty.1", ""),
            text("usr/share/man/man1/zero.1", "0\n"),
            text("usr/share/man/man1/short.1", &"\n".repeat(255)),
            text("usr/share/man/de/man1/empty.1", ""),
            (
                "usr/share/man/man1/hard.1".to_string(),
                TestFile::new("").hardlink(),
            ),
            gzip_empty("usr/share/man/man6/games.6.gz"),
            // Named `.gz` but not gzip data: the bytes themselves are empty.
            text("usr/share/man/man1/touched.1.gz", ""),
        ])];
        BinaryPackageTester::new(EmptyManualPage, pass, fail).test_and_snapshot();
    }
}
