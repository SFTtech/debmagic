use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::source_missing::{prefix, visited_files};

pub struct SourceContainsPrebuiltWasmBinary;

declare_rule! {
    /// A Patched file is a prebuilt WebAssembly binary: a module that starts
    /// with the `\0asm` magic, which `file(1)` reads as `WebAssembly (wasm)
    /// binary`. Reported at pedantic severity with an empty extra and the
    /// path as the Location, the pointer form lintian's
    /// `files/source-missing` Check emits.
    ///
    /// Walks the packaging File tree: hardlinks are read as files, quilt
    /// `.pc/` and `debian/missing-sources/` are skipped.
    ///
    /// Differs from lintian: the file type is the magic number alone, not
    /// libmagic's description.
    SourceContainsPrebuiltWasmBinary,
    code = "LN1629",
    tag = "source-contains-prebuilt-wasm-binary",
    default_selected = false,
    experimental = false,
}

/// The `\0asm` magic every WebAssembly module starts with, whatever its version.
fn is_wasm(prefix: &[u8]) -> bool {
    prefix.starts_with(b"\0asm")
}

impl SourceRule for SourceContainsPrebuiltWasmBinary {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let flagged: Vec<_> = visited_files(files)
            .filter(|entry| is_wasm(&prefix(files, entry)))
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

    /// The WebAssembly magic and version 1.
    const WASM: &[u8] = b"\0asm\x01\x00\x00\x00";

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("src/notes.txt", TestFile::new("asm is not a module\n")),
                // Quilt's `.pc/` is not the packaging's file.
                (".pc/fake.wasm", TestFile::from_bytes(WASM)),
                // The Check does not read `debian/missing-sources/`.
                (
                    "debian/missing-sources/fake.wasm",
                    TestFile::from_bytes(WASM),
                ),
                // A directory named like a module is not a file.
                ("src/dir.wasm", TestFile::directory()),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("fake.wasm", TestFile::from_bytes(WASM)),
            ],
            // A hardlink is a file too.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("fake.wasm", TestFile::from_bytes(WASM).hardlink()),
            ],
        ];
        SourceTreeTester::new(SourceContainsPrebuiltWasmBinary, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::from(CONTROL),
        )])];
        let fail = vec![SourcePackageCase::from(vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("fake.wasm", TestFile::from_bytes(WASM)),
        ])];
        SourcePackageTester::new(SourceContainsPrebuiltWasmBinary, pass, fail).test_and_snapshot();
    }
}
