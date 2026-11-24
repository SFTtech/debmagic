use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::{ElfFiles, ElfObjectType};
use crate::lint::rule::BinaryPackageRule;

pub struct DebugSymbolsDirectlyInUsrLibDebug;

declare_rule! {
    /// A Binary package installs an ELF executable or shared object directly
    /// in `usr/lib/debug/`, not in a subdirectory of it. Detached debugging
    /// symbols belong below `usr/lib/debug/` in a directory that mirrors the
    /// path of the file they belong to.
    ///
    /// Walks the ELF facts of Data files (regular files and hardlinks) whose
    /// object type `file(1)` would call an executable or a shared object,
    /// pie executables included. Relocatable objects are not hits. Extra is
    /// empty; the Data files path is the Location. Not skipped for udebs,
    /// like lintian. Lintian's `file(1)` ELF test is replaced by the
    /// object type of `ElfFacts`, so an ELF whose header is damaged is not
    /// seen.
    DebugSymbolsDirectlyInUsrLibDebug,
    code = "LN1008",
    tag = "debug-symbols-directly-in-usr-lib-debug",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebugSymbolsDirectlyInUsrLibDebug {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for path in direct_debug_paths(&elf_files) {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn direct_debug_paths(elf_files: &ElfFiles) -> Vec<std::path::PathBuf> {
    elf_files
        .iter()
        .filter(|(path, facts)| {
            matches!(
                facts.object_type(),
                ElfObjectType::Executable | ElfObjectType::SharedObject
            ) && path.parent() == Some(Path::new("usr/lib/debug"))
        })
        .map(|(path, _)| path.to_path_buf())
        .collect()
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
            // Not a detached symbol file in the wrong place: a subdirectory.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/bin/hello",
                elf(TestElf::executable()),
            )]),
            // Relocatable objects are not executables or shared objects.
            BinaryPackageCase::from(vec![("usr/lib/debug/foo.o", elf(TestElf::relocatable()))]),
            BinaryPackageCase::from(vec![("usr/lib/debug/notes.txt", TestFile::new("text\n"))]),
            BinaryPackageCase::from(vec![("usr/lib/debug/link", TestFile::symlink("basic"))]),
            // Outside usr/lib/debug/.
            BinaryPackageCase::from(vec![("usr/bin/hello", elf(TestElf::pie_executable()))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debugging/hello",
                elf(TestElf::executable()),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/lib/debug/basic", elf(TestElf::executable()))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/libhello.so.0.0",
                elf(TestElf::shared_object().soname("libhello.so.0")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/basic.static",
                elf(TestElf::executable()),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/hello",
                elf(TestElf::pie_executable()),
            )]),
            // A hardlink counts as a file, like lintian's `is_file`.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/basic-link",
                elf(TestElf::executable()).hardlink(),
            )]),
            // udebs are not skipped.
            BinaryPackageCase::from(vec![("usr/lib/debug/basic", elf(TestElf::executable()))])
                .udeb(),
        ];
        BinaryPackageTester::new(DebugSymbolsDirectlyInUsrLibDebug, pass, fail).test_and_snapshot();
    }
}
