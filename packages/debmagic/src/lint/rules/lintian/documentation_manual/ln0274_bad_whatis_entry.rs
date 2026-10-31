use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::content;

pub struct BadWhatisEntry;

declare_rule! {
    /// `lexgrog` could not parse the manual page's `NAME` section.
    ///
    /// A regular file or hardlink under `usr/share/man/manN/` (the path
    /// matches `/man/manN/`). Gzip magic `1f 8b` stays compressed for
    /// `lexgrog`. A page under 256 bytes whose opening line is empty or a
    /// `.so` request is left alone. Exit status 2 is this Tag. Other
    /// variables are cleared except `PATH` and `TMPDIR`, and `LC_ALL` is
    /// `C.UTF-8`. Locale directories are not scanned. Udebs are skipped.
    /// Extra is empty; the Data files path is the Location.
    BadWhatisEntry,
    code = "LN0274",
    tag = "bad-whatis-entry",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BadWhatisEntry {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for page in content::pages(files) {
            if page.opening.stops() || !page.english_hierarchy {
                continue;
            }
            let Some(whatis) = content::whatis(&page) else {
                continue;
            };
            if whatis.bad() {
                hits.push(page.path);
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
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

    const BAD: &str = ".TH PROGRAM 1 \"2013-07-22\"\n.SH NAME\nbad-whatis\n";
    const CLEAN: &str = ".TH FOO 1\n.SH NAME\nfoo \\- frobnicate files\n";

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

    fn padded_bad() -> String {
        let mut text = BAD.to_string();
        while text.len() < 256 {
            text.push_str(".\\\" pad\n");
        }
        text
    }

    #[test]
    fn test() {
        let pass = vec![
            case(vec![]),
            case(vec![text("usr/share/man/man1/clean.1", CLEAN)]),
            // Not the English hierarchy, even though lexgrog would fail.
            case(vec![text("usr/share/man/de/man1/bad.1", BAD)]),
            // A short `.so` returns before lexgrog.
            case(vec![text("usr/share/man/man1/link.1", ".so man1/foo.1\n")]),
            case(vec![text("usr/share/man/man1/empty.1", "")]),
            case(vec![(
                "usr/share/man/man1/bad.1".to_string(),
                TestFile::symlink("other.1"),
            )]),
            case(vec![text("usr/share/man/man1/bad.1", BAD)]).udeb(),
        ];
        let fail = vec![case(vec![
            text("usr/share/man/man1/bad.1", BAD),
            text("usr/share/man/man/man1/nested.1", BAD),
            text("usr/share/man/man1/padded.1", &padded_bad()),
            (
                "usr/share/man/man1/hard.1".to_string(),
                TestFile::new(BAD).hardlink(),
            ),
            gzip("usr/share/man/man1/bad.1.gz", BAD),
        ])];
        BinaryPackageTester::new(BadWhatisEntry, pass, fail).test_and_snapshot();
    }
}
