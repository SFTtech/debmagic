use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::{ElfFiles, ElfObjectType};
use crate::lint::rule::BinaryPackageRule;

pub struct DebugSymbolsNotDetached;

declare_rule! {
    /// A Binary package installs a detached debugging symbol file under
    /// `usr/lib/debug/` (in a `lib*`, `*bin`, `usr`, `opt`, `dev`, `emul` or
    /// `.build-id` directory) that still has a dynamic `NEEDED` entry. Such a
    /// file is a copy of the binary's code, not a debug-only copy, so it was
    /// not split off with `objcopy --only-keep-debug` and its dependencies
    /// are still declared.
    ///
    /// Walks the ELF facts of Data files (regular files and hardlinks) whose
    /// object type `file(1)` would call an executable or a shared object. The
    /// path must match `usr/lib/debug/` followed by one of those directory
    /// names and a `/`. A file is a hit when its dynamic section has at least
    /// one `NEEDED`. Extra is empty; the Data files path is the Location. Not
    /// skipped for udebs, like lintian. Lintian's `file(1)` ELF test is
    /// replaced by the object type of `ElfFacts`, so an ELF whose header is
    /// damaged is not seen.
    DebugSymbolsNotDetached,
    code = "LN1009",
    tag = "debug-symbols-not-detached",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebugSymbolsNotDetached {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for path in not_detached_paths(&elf_files) {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn not_detached_paths(elf_files: &ElfFiles) -> Vec<PathBuf> {
    elf_files
        .iter()
        .filter(|(path, facts)| {
            matches!(
                facts.object_type(),
                ElfObjectType::Executable | ElfObjectType::SharedObject
            ) && !facts.needed().is_empty()
                && path.to_str().is_some_and(|name| DEBUG_TREE.is_match(name))
        })
        .map(|(path, _)| path.to_path_buf())
        .collect()
}

static DEBUG_TREE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib/debug/(?:lib\d*|s?bin|usr|opt|dev|emul|\.build-id)/")
        .expect("debug tree regex")
});

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
            // A debug-only copy: no dynamic NEEDED.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/bin/hello",
                elf(TestElf::executable().interpreter("")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/lib/libhello.so.0.0",
                elf(TestElf::shared_object().soname("libhello.so.0")),
            )]),
            // Outside the directories lintian looks at.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/foo/bar",
                elf(TestElf::shared_object().needed("libc.so.6")),
            )]),
            // Directly in usr/lib/debug/ is LN1008's business, not this one.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/basic",
                elf(TestElf::executable().needed("libc.so.6")),
            )]),
            // Not an executable or shared object.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/lib/foo.o",
                elf(TestElf::relocatable().needed("libc.so.6")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/bin/notes",
                TestFile::new("text\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/bin/link",
                TestFile::symlink("hello"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/bin/hello",
                elf(TestElf::pie_executable().needed("libc.so.6")),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/bin/hello",
                elf(TestElf::pie_executable().needed("libc.so.6")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/lib/libhello.so.0.0",
                elf(TestElf::shared_object()
                    .soname("libhello.so.0")
                    .needed("libc.so.6")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/.build-id/de/adbeef.debug",
                elf(TestElf::executable().needed("libc.so.6")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/opt/tool",
                elf(TestElf::executable().needed("libc.so.6")),
            )]),
            // A hardlink counts as a file, like lintian's `is_file`.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/bin/hello-link",
                elf(TestElf::executable().needed("libc.so.6")).hardlink(),
            )]),
            // udebs are not skipped.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/bin/hello",
                elf(TestElf::executable().needed("libc.so.6")),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(DebugSymbolsNotDetached, pass, fail).test_and_snapshot();
    }
}
