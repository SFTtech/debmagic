use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Entry;
use crate::lint::rule::{Severity, SourceRule};

use super::source_missing::{is_composite_document, prefix, visited_files};

pub struct SourceContainsPrebuiltFlashProject;

declare_rule! {
    /// A Patched file is a Shockwave Flash project (an `.fla` file): an OLE2
    /// compound document named `*.fla`, which developers often leave in a
    /// tarball by mistake. Reported at pedantic severity with an empty extra
    /// and the path as the Location, the pointer form lintian's
    /// `files/source-missing` Check emits.
    ///
    /// The name is matched without regard to case. Walks the packaging File
    /// tree: hardlinks are read as files, quilt `.pc/` is skipped, and
    /// `debian/missing-sources/` is skipped. The file's type is the OLE2
    /// magic number, which `file(1)` reads as a composite document.
    ///
    /// Differs from lintian: the file type is the magic number alone, not
    /// libmagic's description.
    SourceContainsPrebuiltFlashProject,
    code = "LN1624",
    tag = "source-contains-prebuilt-flash-project",
    default_selected = false,
    experimental = false,
}

/// lintian's `$item->name =~ m{(?i)\.fla$}x`.
fn is_fla_name(entry: &Entry) -> bool {
    entry
        .path()
        .to_string_lossy()
        .to_lowercase()
        .ends_with(".fla")
}

impl SourceRule for SourceContainsPrebuiltFlashProject {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let flagged: Vec<_> = visited_files(files)
            .filter(|entry| is_fla_name(entry) && is_composite_document(&prefix(files, entry)))
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

    /// The OLE2 compound document magic, then padding.
    const OLE2: &[u8] = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1\x00\x00\x00\x00";

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("src/notes.txt", TestFile::new("not a project\n")),
                // A compound document is not a project unless it is named `.fla`.
                ("src/icon.psd.bin", TestFile::from_bytes(OLE2)),
                // A project file kept by quilt's `.pc/` is not the packaging's file.
                (".pc/fakefla.fla", TestFile::from_bytes(OLE2)),
                // A named project with other content is not a Flash project.
                ("src/fake.fla", TestFile::new("FWS\x06")),
                // A directory named like a project is not a file.
                ("src/dir.fla", TestFile::directory()),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("fakefla.fla", TestFile::from_bytes(OLE2)),
            ],
            // The name is matched without regard to case.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("src/Sources.FLA", TestFile::from_bytes(OLE2)),
            ],
            // A hardlink is a file too.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("fakefla.fla", TestFile::from_bytes(OLE2).hardlink()),
            ],
        ];
        SourceTreeTester::new(SourceContainsPrebuiltFlashProject, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::from(CONTROL),
        )])];
        let fail = vec![SourcePackageCase::from(vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("fakefla.fla", TestFile::from_bytes(OLE2)),
        ])];
        SourcePackageTester::new(SourceContainsPrebuiltFlashProject, pass, fail)
            .test_and_snapshot();
    }
}
