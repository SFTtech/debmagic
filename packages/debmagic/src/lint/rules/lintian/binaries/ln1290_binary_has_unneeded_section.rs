use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::ElfFiles;
use crate::lint::rule::BinaryPackageRule;

pub struct BinaryHasUnneededSection;

declare_rule! {
    /// An ELF object that is stripped still has a `.note` or `.comment`
    /// section. `strip --remove-section=.comment --remove-section=.note`
    /// removes them; `dh_strip` does that, `install -s` does not.
    ///
    /// Walks the ELF facts of Data files (regular files and hardlinks) whose
    /// section names include `.note` or `.comment`. An object with a
    /// `.symtab` (lintian's "not stripped") is skipped, as are files under
    /// `lib/debug/` and `lib/profile/` (also under `usr/`). Each distinct
    /// known section name is one hit: the extra is the section name and the
    /// Data files path is the Location. Not skipped for udebs, like lintian.
    /// Lintian's `file(1)` "not stripped" test is replaced by the `.symtab`
    /// section, which `readelf` shows for the same objects.
    BinaryHasUnneededSection,
    code = "LN1290",
    tag = "binary-has-unneeded-section",
    default_selected = false,
    experimental = false,
}

const KNOWN_STRIPPED_SECTION_NAMES: [&str; 2] = [".note", ".comment"];

impl BinaryPackageRule for BinaryHasUnneededSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for (path, extra) in unneeded_sections(&elf_files) {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// The `(path, section name)` hits, in path order, then in the order of the
/// known names.
fn unneeded_sections(elf_files: &ElfFiles) -> Vec<(std::path::PathBuf, &'static str)> {
    let mut hits = Vec::new();
    for (path, facts) in elf_files.iter() {
        if is_debug_or_profile(path) {
            continue;
        }
        let names = facts.section_names();
        if names.iter().any(|name| name == ".symtab") {
            continue;
        }
        for known in KNOWN_STRIPPED_SECTION_NAMES {
            if names.iter().any(|name| name == known) {
                hits.push((path.to_path_buf(), known));
            }
        }
    }
    hits
}

/// lintian's `^ (?:usr/)? lib/ (?: debug | profile ) /`.
fn is_debug_or_profile(path: &Path) -> bool {
    let path = path.strip_prefix("usr").unwrap_or(path);
    path.starts_with("lib/debug") || path.starts_with("lib/profile")
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
            // Nothing to remove.
            BinaryPackageCase::from(vec![("usr/bin/hello", elf(TestElf::executable()))]),
            // Not stripped: a symbol table is still there.
            BinaryPackageCase::from(vec![(
                "usr/bin/hello",
                elf(TestElf::executable().section(".symtab").section(".comment")),
            )]),
            // Debugging and profiling objects keep their sections.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/bin/hello",
                elf(TestElf::executable().section(".comment")),
            )]),
            BinaryPackageCase::from(vec![(
                "lib/profile/libhello.so",
                elf(TestElf::shared_object().section(".note")),
            )]),
            // Not an ELF object.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/hello/notes",
                TestFile::new(".comment\n"),
            )]),
            // Other sections are not stripped sections.
            BinaryPackageCase::from(vec![(
                "usr/bin/hello",
                elf(TestElf::executable().section(".debug_line")),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/bin/hello",
                elf(TestElf::executable().section(".comment")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libhello.so.0",
                elf(TestElf::shared_object()
                    .section(".note")
                    .section(".comment")),
            )]),
            // A hardlink counts as a file, like lintian's `is_file`.
            BinaryPackageCase::from(vec![(
                "usr/bin/hello-link",
                elf(TestElf::executable().section(".comment")).hardlink(),
            )]),
            // udebs are not skipped.
            BinaryPackageCase::from(vec![(
                "usr/bin/hello",
                elf(TestElf::executable().section(".comment")),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(BinaryHasUnneededSection, pass, fail).test_and_snapshot();
    }
}
