use std::path::PathBuf;

use super::source_missing::is_elf;
use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::{Severity, SourceRule};

pub struct SourceContainsPrebuiltBinary;

declare_rule! {
    /// The Source tree or Source package contains a prebuilt ELF object: a
    /// regular file or hardlink whose File type is ELF (lintian's `\bELF\b`
    /// on `file(1)` output). Reported at pedantic severity, with the Entry path
    /// as the Location and an empty extra.
    ///
    /// Walks the packaging File tree (Source tree, or Source package Patched
    /// files). Quilt `.pc/` is filtered out, and `debian/missing-sources/` is
    /// skipped. The ELF test is `file(1)`'s `ELF`: debmagic's File type, or,
    /// where its headers do not parse, the ELF magic (see `source_missing`).
    ///
    /// The sibling `source-is-missing` (LN1390) reports the same files when no
    /// source for them is in the tree.
    SourceContainsPrebuiltBinary,
    code = "LN1387",
    tag = "source-contains-prebuilt-binary",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsPrebuiltBinary {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        for path in elf_paths(files) {
            // The oracle hint is the bare `[path]` pointer, so the extra is empty.
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

fn elf_paths(files: &FileTree) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter(|entry| {
            let path = entry.path();
            !path.starts_with(".pc")
                && !path.starts_with("debian/missing-sources")
                && is_elf(files, entry)
        })
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::file_type::FileType;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const ELF: FileType = FileType::Elf {
        has_static_symbol_table: false,
        executable: true,
    };

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("bin/tool.sh", TestFile::new("#!/bin/sh\n")),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (".pc/tool.o", TestFile::new("ignored").file_type(ELF)),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "debian/missing-sources/tool.o",
                    TestFile::new("sourced").file_type(ELF),
                ),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("bin/tool", TestFile::symlink("tool.real")),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("bin/tool", TestFile::new("elf").file_type(ELF)),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("lib/libfoo.so.1", TestFile::new("elf").file_type(ELF)),
            ],
            // A hardlink is a file too.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("bin/tool", TestFile::new("elf").file_type(ELF).hardlink()),
            ],
            // Corrupt section headers: no File type, but file(1) still says ELF.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "tests/invalid-elf/bad",
                    TestFile::from_bytes(b"\x7fELF\x02\x01\x01 corrupt headers".to_vec()),
                ),
            ],
        ];
        SourceTreeTester::new(SourceContainsPrebuiltBinary, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::from(CONTROL),
        )])];
        let fail = vec![SourcePackageCase::from(vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("bin/tool", TestFile::new("elf").file_type(ELF)),
        ])];
        SourcePackageTester::new(SourceContainsPrebuiltBinary, pass, fail).test_and_snapshot();
    }
}
