use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::ln0722_arch_dependent_file_not_in_arch_specific_directory::is_static_archive;

pub struct ArchDependentFileInUsrShare;

declare_rule! {
    /// An ELF object or static archive is installed under `usr/share/`.
    ///
    /// Walks Data files that are regular files or hardlinks, whose path starts
    /// with `usr/share/`, and that are ELF (any `ELF` file, as the ELF facts
    /// see it) or a static archive (`!<arch>` magic that is not a `.deb`). It
    /// does not depend on Architecture or Multi-Arch. Extra is empty; the Data
    /// files path is the Location.
    ArchDependentFileInUsrShare,
    code = "LN0737",
    tag = "arch-dependent-file-in-usr-share",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ArchDependentFileInUsrShare {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter(|entry| entry.path().to_string_lossy().starts_with("usr/share/"))
            .filter(|entry| {
                elf_files.get(entry.path()).is_some()
                    || files
                        .read(entry)
                        .is_ok_and(|bytes| is_static_archive(&bytes))
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    fn archive() -> TestFile {
        TestFile::from_bytes(
            b"!<arch>\nlibfoo.o/      0           0     0     644     4         `\nabcd".to_vec(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Only usr/share/ counts.
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("var/share/foo/basic", elf(TestElf::executable()))])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/sharefoo/basic", elf(TestElf::executable()))])
                .field("Architecture", "amd64"),
            // Not an ELF object or a static archive.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/foo/notes",
                TestFile::new("#!/bin/sh\n"),
            )])
            .field("Architecture", "all"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/foo/notes",
                TestFile::new("!<arch>text"),
            )])
            .field("Architecture", "all"),
            // A .deb is not a static archive.
            BinaryPackageCase::from(vec![(
                "usr/share/foo/bar.deb",
                TestFile::from_bytes(b"!<arch>\ndebian-binary   ".to_vec()),
            )])
            .field("Architecture", "all"),
            // Symbolic links are not regular files.
            BinaryPackageCase::from(vec![(
                "usr/share/foo/link",
                TestFile::symlink("/usr/lib/x86_64-linux-gnu/libfoo.so.1"),
            )])
            .field("Architecture", "all"),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/share/foo/basic", elf(TestElf::executable()))])
                .field("Architecture", "all"),
            // Architecture or triplet directories do not exempt a file.
            BinaryPackageCase::from(vec![(
                "usr/share/x86_64-linux-gnu/foo/libfoo.so.1",
                elf(TestElf::shared_object()),
            )])
            .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/share/foo/libfoo.a", archive())])
                .field("Architecture", "all"),
            BinaryPackageCase::from(vec![(
                "usr/share/foo/same-link",
                elf(TestElf::executable()).hardlink_to("usr/share/foo/same-target"),
            )])
            .field("Architecture", "amd64"),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![("usr/share/foo/basic", elf(TestElf::executable()))])
                .field("Architecture", "amd64")
                .udeb(),
        ];
        BinaryPackageTester::new(ArchDependentFileInUsrShare, pass, fail).test_and_snapshot();
    }
}
