use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::elf::{ElfFiles, ElfObjectType};
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::fields_relations_more::architectures::deb_host_multiarch_triplets;

pub struct SharedLibraryIsMultiArchForeign;

declare_rule! {
    /// A `Multi-Arch: foreign` Binary package installs a shared library in a
    /// public library directory.
    ///
    /// Only for a Binary package whose `Multi-Arch` field is `foreign`. Walks
    /// Data files regular files and hardlinks whose ELF object type is `file(1)`'s
    /// "shared object" or "pie executable" and whose parent directory is an
    /// `ldconfig` folder: `lib`, `lib32`,
    /// `lib64`, `libx32`, `usr/lib`, `usr/lib32`, `usr/lib64`, `usr/libx32`,
    /// `usr/local/lib`, and `lib/<triplet>` and `usr/lib/<triplet>` for every
    /// `DEB_HOST_MULTIARCH` triplet. The hit paths are sorted, de-duplicated
    /// and joined with a space into one Diagnostic with no Location (lintian's
    /// hint has no pointer). Udebs and ddebs are checked too.
    SharedLibraryIsMultiArchForeign,
    code = "LN1127",
    tag = "shared-library-is-multi-arch-foreign",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SharedLibraryIsMultiArchForeign {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let multi_arch = ctx
            .installation_control()
            .fields()
            .get("Multi-Arch")
            .unwrap_or_default()
            .trim()
            .to_string();
        if multi_arch != "foreign" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let folders = ldconfig_folders();
        let candidates: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_file())
            .filter(|entry| {
                entry
                    .path()
                    .parent()
                    .is_some_and(|dir| folders.iter().any(|folder| folder == dir))
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        if candidates.is_empty() {
            return;
        }
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        let mut names: Vec<String> = candidates
            .into_iter()
            .filter(|path| is_shared_object(&elf_files, path))
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        if names.is_empty() {
            return;
        }
        names.sort();
        names.dedup();
        ctx.diagnostic(Diagnostic::error(names.join(" ")));
    }
}

/// `file(1)` "shared object" (`ET_DYN` without `DF_1_PIE`) or "pie executable"
/// (`ET_DYN` with `DF_1_PIE`).
fn is_shared_object(elf_files: &ElfFiles, path: &Path) -> bool {
    elf_files.get(path).is_some_and(|facts| {
        facts.object_type() == ElfObjectType::SharedObject
            || (facts.object_type() == ElfObjectType::Executable && facts.is_pie())
    })
}

/// Lintian's `ldconfig_folders`: the multiarch and always-listed library
/// directories, without trailing slashes.
fn ldconfig_folders() -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = deb_host_multiarch_triplets()
        .into_iter()
        .flat_map(|triplet| [format!("lib/{triplet}"), format!("usr/lib/{triplet}")])
        .map(PathBuf::from)
        .collect();
    folders.extend(
        [
            "lib",
            "lib32",
            "lib64",
            "libx32",
            "usr/lib",
            "usr/lib32",
            "usr/lib64",
            "usr/libx32",
            "usr/local/lib",
        ]
        .map(PathBuf::from),
    );
    folders
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Only Multi-Arch: foreign is checked.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .field("Multi-Arch", "allowed"),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )]),
            // Outside an ldconfig folder.
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .field("Multi-Arch", "foreign"),
            // Not a shared object.
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                elf(TestElf::executable().needed("libc.so.6")),
            )])
            .field("Multi-Arch", "foreign"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.a", TestFile::new("!<arch>\n"))])
                .field("Multi-Arch", "foreign"),
            BinaryPackageCase::from(vec![(
                "usr/lib/link.so.1",
                TestFile::symlink("libfoo.so.1"),
            )])
            .field("Multi-Arch", "foreign"),
            BinaryPackageCase::from(vec![("usr/lib/foo.o", elf(TestElf::relocatable()))])
                .field("Multi-Arch", "foreign"),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .field("Multi-Arch", "foreign"),
            // PIE executable, and a multiarch library directory.
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/libpie.so.1",
                elf(TestElf::pie_executable().soname("libpie.so.1")),
            )])
            .field("Multi-Arch", "foreign"),
            BinaryPackageCase::from(vec![
                (
                    "lib/x86_64-linux-gnu/libb.so.1",
                    elf(TestElf::shared_object().soname("libb.so.1")),
                ),
                (
                    "usr/lib/liba.so.1",
                    elf(TestElf::shared_object().soname("liba.so.1")),
                ),
            ])
            .field("Multi-Arch", "foreign"),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")).hardlink(),
            )])
            .field("Multi-Arch", "foreign"),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .field("Multi-Arch", "foreign")
            .udeb(),
        ];
        BinaryPackageTester::new(SharedLibraryIsMultiArchForeign, pass, fail).test_and_snapshot();
    }
}
