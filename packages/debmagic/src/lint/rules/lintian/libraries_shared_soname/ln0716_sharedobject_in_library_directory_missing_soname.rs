use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::{ElfFacts, ElfObjectType};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::fields_relations_more::architectures::is_ldconfig_folder;

pub struct SharedobjectInLibraryDirectoryMissingSoname;

declare_rule! {
    /// A shared object in a library directory has no SONAME.
    ///
    /// Walks the ELF facts of Data files (regular files and hardlinks) that
    /// `file(1)` would call a "shared object" (`ET_DYN` without `DF_1_PIE`) or
    /// a "pie executable" (`ET_DYN` with `DF_1_PIE`). Such a file is a hit when
    /// its directory is one of lintian's `ldconfig_folders` (`lib`, `usr/lib`,
    /// `usr/local/lib`, and the multiarch directories of every release
    /// architecture; subdirectories are not included) and its dynamic section
    /// has no `DT_SONAME`. A PIE that is executable, has `DT_DEBUG`, and whose
    /// name has no `.so` infix or suffix is not a hit. Extra is empty; the
    /// Data files path is the Location.
    SharedobjectInLibraryDirectoryMissingSoname,
    code = "LN0716",
    tag = "sharedobject-in-library-directory-missing-soname",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SharedobjectInLibraryDirectoryMissingSoname {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        let paths: Vec<PathBuf> = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            elf_files
                .iter()
                .filter(|(path, facts)| is_hit(path, facts, files))
                .map(|(path, _)| path.to_path_buf())
                .collect()
        };
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn is_hit(path: &Path, facts: &ElfFacts, files: &FileTree) -> bool {
    let Some(entry) = files.get(path) else {
        return false;
    };
    let shared = match facts.object_type() {
        ElfObjectType::SharedObject => true,
        ElfObjectType::Executable => facts.is_pie(),
        ElfObjectType::Relocatable | ElfObjectType::Other => false,
    };
    if !shared || !facts.soname().is_empty() {
        return false;
    }
    let dir = path.parent().and_then(Path::to_str).unwrap_or_default();
    if !is_ldconfig_folder(dir) {
        return false;
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let library_name = name.contains(".so.") || name.ends_with(".so");
    !(entry.is_executable_file() && facts.has_debug_entry() && !library_name)
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
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )]),
            // Only the directory itself is a library directory, not its subdirectories.
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/libbar.so",
                elf(TestElf::shared_object()),
            )]),
            BinaryPackageCase::from(vec![("opt/lib/libbar.so", elf(TestElf::shared_object()))]),
            BinaryPackageCase::from(vec![("usr/bin/libbar.so", elf(TestElf::shared_object()))]),
            // Not a shared object: an ET_EXEC or a relocatable object.
            BinaryPackageCase::from(vec![("usr/lib/helper", elf(TestElf::executable()))]),
            BinaryPackageCase::from(vec![("usr/lib/foo.o", elf(TestElf::relocatable()))]),
            BinaryPackageCase::from(vec![("usr/lib/script", TestFile::new("#!/bin/sh\n"))]),
            BinaryPackageCase::from(vec![("usr/lib/link", TestFile::symlink("libfoo.so"))]),
            // A PIE executable with DT_DEBUG that is executable and not a library.
            BinaryPackageCase::from(vec![(
                "usr/lib/helper",
                elf(TestElf::pie_executable()
                    .interpreter("/lib64/ld-linux-x86-64.so.2")
                    .needed("libc.so.6")
                    .debug())
                .mode(0o755),
            )]),
            // Multiarch directories with a SONAME.
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/lib/libhallo.so", elf(TestElf::shared_object()))]),
            BinaryPackageCase::from(vec![("lib/libhallo.so.1", elf(TestElf::shared_object()))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/libbaz.so.1.0",
                elf(TestElf::shared_object()),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/local/lib/libbaz.so.1",
                elf(TestElf::shared_object()),
            )]),
            // A dotted name without `.so` is still a shared object.
            BinaryPackageCase::from(vec![(
                "usr/lib/libbaz1.so.1.0.3b",
                elf(TestElf::shared_object()),
            )]),
            // A PIE without DT_DEBUG is a hit even when executable.
            BinaryPackageCase::from(vec![(
                "usr/lib/helper",
                elf(TestElf::pie_executable().needed("libc.so.6")).mode(0o755),
            )]),
            // A PIE with DT_DEBUG is a hit when not executable.
            BinaryPackageCase::from(vec![(
                "usr/lib/helper",
                elf(TestElf::pie_executable().needed("libc.so.6").debug()).mode(0o644),
            )]),
            // A library name is a hit even when executable with DT_DEBUG.
            BinaryPackageCase::from(vec![(
                "usr/lib/libhelper.so",
                elf(TestElf::pie_executable().needed("libc.so.6").debug()).mode(0o755),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libhelper.so.dbg",
                elf(TestElf::shared_object()).mode(0o755),
            )]),
            // udebs are not exempt.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", elf(TestElf::shared_object()))])
                .udeb(),
        ];
        BinaryPackageTester::new(SharedobjectInLibraryDirectoryMissingSoname, pass, fail)
            .test_and_snapshot();
    }
}
