use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct ArchIndependentPackageContainsBinaryOrObject;

declare_rule! {
    /// A binary package with `Architecture: all` installs an ELF object or a static archive.
    ///
    /// Walks Data files that are regular files or hardlinks and reports each
    /// one that is ELF (any ELF file, as the ELF facts see it) or a static
    /// archive (`!<arch>` magic that is not a `.deb`, as `file(1)` calls a
    /// `current ar archive`). Applies only when the Installation control
    /// `Architecture` is exactly `all`; udebs are not skipped. Extra is empty;
    /// the Data files path is the Location. Unlike lintian, the magic is
    /// sniffed directly rather than matched against `file(1)` output.
    ArchIndependentPackageContainsBinaryOrObject,
    code = "LN1005",
    tag = "arch-independent-package-contains-binary-or-object",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ArchIndependentPackageContainsBinaryOrObject {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let architecture = ctx
            .installation_control()
            .fields()
            .unfolded_value("Architecture");
        if architecture.as_deref() != Some("all") {
            return;
        }
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

/// `!<arch>` magic of a static archive; a `.deb` is an ar archive too but
/// `file(1)` does not call it one.
fn is_static_archive(bytes: &[u8]) -> bool {
    bytes
        .strip_prefix(b"!<arch>\n")
        .is_some_and(|members| !members.starts_with(b"debian-binary"))
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
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Architecture", "all"),
            // Only `all` is checked.
            BinaryPackageCase::from(vec![("usr/bin/foo", elf(TestElf::executable()))])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.a", archive())])
                .field("Architecture", "any"),
            // Not an ELF object or a static archive.
            BinaryPackageCase::from(vec![("usr/share/foo/data", TestFile::new("text"))])
                .field("Architecture", "all"),
            BinaryPackageCase::from(vec![("usr/share/foo/notes", TestFile::new("!<arch>text"))])
                .field("Architecture", "all"),
            // A .deb is not a static archive.
            BinaryPackageCase::from(vec![(
                "usr/share/foo/bar.deb",
                TestFile::from_bytes(b"!<arch>\ndebian-binary   ".to_vec()),
            )])
            .field("Architecture", "all"),
            // Symbolic links are not regular files.
            BinaryPackageCase::from(vec![("usr/bin/link", TestFile::symlink("/usr/bin/foo"))])
                .field("Architecture", "all"),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/bin/foo", elf(TestElf::executable()))])
                .field("Architecture", "all"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", elf(TestElf::shared_object()))])
                .field("Architecture", "all"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.a", archive())])
                .field("Architecture", "all"),
            // A hardlink to an ELF object counts.
            BinaryPackageCase::from(vec![(
                "usr/bin/foo-link",
                elf(TestElf::executable()).hardlink_to("usr/bin/foo"),
            )])
            .field("Architecture", "all"),
            // Any path counts, not only under `usr/`.
            BinaryPackageCase::from(vec![("etc/foo", elf(TestElf::executable()))])
                .field("Architecture", "all"),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![("usr/bin/foo", elf(TestElf::executable()))])
                .field("Architecture", "all")
                .udeb(),
        ];
        BinaryPackageTester::new(ArchIndependentPackageContainsBinaryOrObject, pass, fail)
            .test_and_snapshot();
    }
}
