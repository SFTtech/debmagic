use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::{ElfFiles, ElfObjectType};
use crate::lint::rule::BinaryPackageRule;

pub struct DebugFileWithNoDebugSymbols;

declare_rule! {
    /// A detached debugging symbol file under `usr/lib/debug/` (in a `lib*`,
    /// `*bin`, `usr`, `opt`, `dev`, `emul` or `.build-id` directory) has none
    /// of the DWARF sections that carry debugging information (`.debug_line`,
    /// `.debug_str`, and their `.zdebug_` forms). Such a file was not built
    /// or split with its debugging data, so `gdb` has nothing to show.
    ///
    /// Walks the ELF facts of Data files (regular files and hardlinks) whose
    /// object type `file(1)` would call an executable or a shared object,
    /// pie executables included. The path must match `usr/lib/debug/`
    /// followed by one of those directory names and a `/`. A file is a hit
    /// when its section names have none of the four DWARF names. Extra is
    /// empty; the Data files path is the Location. Not skipped for udebs,
    /// like lintian. Lintian's `file(1)` ELF test is replaced by the object
    /// type of `ElfFacts`, so an ELF whose header is damaged is not seen.
    DebugFileWithNoDebugSymbols,
    code = "LN1293",
    tag = "debug-file-with-no-debug-symbols",
    default_selected = true,
    experimental = false,
}

const DEBUG_SECTION_NAMES: [&str; 4] = [".debug_line", ".zdebug_line", ".debug_str", ".zdebug_str"];

static DEBUG_TREE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib/debug/(?:lib\d*|s?bin|usr|opt|dev|emul|\.build-id)/")
        .expect("debug tree regex")
});

impl BinaryPackageRule for DebugFileWithNoDebugSymbols {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for path in files_without_debug_symbols(&elf_files) {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn files_without_debug_symbols(elf_files: &ElfFiles) -> Vec<PathBuf> {
    elf_files
        .iter()
        .filter(|(path, facts)| {
            matches!(
                facts.object_type(),
                ElfObjectType::Executable | ElfObjectType::SharedObject
            ) && DEBUG_TREE.is_match(&path.to_string_lossy())
                && !facts
                    .section_names()
                    .iter()
                    .any(|name| DEBUG_SECTION_NAMES.iter().any(|debug| name == debug))
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
            // Debugging information is there, in either form.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/bin/hello",
                elf(TestElf::executable().section(".debug_line")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/lib/libhello.so.0",
                elf(TestElf::shared_object().section(".zdebug_str")),
            )]),
            // Not in a detached debug directory: the Check does not look.
            BinaryPackageCase::from(vec![("usr/lib/debug/hello", elf(TestElf::executable()))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debugging/hello",
                elf(TestElf::executable()),
            )]),
            BinaryPackageCase::from(vec![("usr/bin/hello", elf(TestElf::pie_executable()))]),
            // Relocatable objects are not executables or shared objects.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/foo.o",
                elf(TestElf::relocatable()),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/notes.txt",
                TestFile::new("text\n"),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/share/foo/basic",
                elf(TestElf::executable()),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/lib/libhello.so.0.0",
                elf(TestElf::shared_object().section(".comment")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/lib/libhello.so.0.0",
                elf(TestElf::shared_object()),
            )]),
            // A hardlink counts as a file, like lintian's `is_file`.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/bin/hello-link",
                elf(TestElf::executable()).hardlink(),
            )]),
            // udebs are not skipped.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/bin/hello",
                elf(TestElf::executable()),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(DebugFileWithNoDebugSymbols, pass, fail).test_and_snapshot();
    }
}
