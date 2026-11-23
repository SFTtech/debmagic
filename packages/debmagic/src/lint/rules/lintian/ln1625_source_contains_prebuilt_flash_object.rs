use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::source_missing::{is_macromedia_flash, prefix, visited_files};

pub struct SourceContainsPrebuiltFlashObject;

declare_rule! {
    /// A Patched file is a prebuilt Shockwave Flash object: a Flash movie
    /// (`FWS`, or compressed `CWS` and `ZWS`) or a Flash video (`FLV`), which
    /// developers often leave in a tarball by mistake. Reported at pedantic
    /// severity with an empty extra and the path as the Location, the pointer
    /// form lintian's `files/source-missing` Check emits.
    ///
    /// Walks the packaging File tree: hardlinks are read as files, quilt
    /// `.pc/` and `debian/missing-sources/` are skipped. The file's type is
    /// its magic number, as `file(1)` reads a Macromedia Flash file.
    ///
    /// Differs from lintian: the file type is the magic number alone, not
    /// libmagic's description.
    SourceContainsPrebuiltFlashObject,
    code = "LN1625",
    tag = "source-contains-prebuilt-flash-object",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsPrebuiltFlashObject {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let flagged: Vec<_> = visited_files(files)
            .filter(|entry| is_macromedia_flash(&prefix(files, entry)))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in flagged {
            ctx.diagnostic(
                Diagnostic::warn("")
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
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const SWF: &[u8] = b"FWS\x06\x10\x00\x00\x00";

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "src/notes.txt",
                    TestFile::new("Flash is not a movie here\n"),
                ),
                // A directory named like a movie is not a file.
                ("src/movie.swf", TestFile::directory()),
                // Quilt's `.pc/` is not the packaging's file.
                (".pc/fakeflash.swf", TestFile::from_bytes(SWF)),
                // The Check does not read `debian/missing-sources/`.
                (
                    "debian/missing-sources/fakeflash.swf",
                    TestFile::from_bytes(SWF),
                ),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("fakeflash.swf", TestFile::from_bytes(SWF)),
            ],
            // A compressed movie and a Flash video are prebuilt too.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("src/compressed.swf", TestFile::from_bytes(b"CWS\x06\x10")),
                ("src/video.flv", TestFile::from_bytes(b"FLV\x01\x05")),
            ],
            // A hardlink is a file too.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("fakeflash.swf", TestFile::from_bytes(SWF).hardlink()),
            ],
        ];
        SourceTreeTester::new(SourceContainsPrebuiltFlashObject, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::from(CONTROL),
        )])];
        let fail = vec![SourcePackageCase::from(vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("fakeflash.swf", TestFile::from_bytes(SWF)),
        ])];
        SourcePackageTester::new(SourceContainsPrebuiltFlashObject, pass, fail).test_and_snapshot();
    }
}
