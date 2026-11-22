use crate::declare_rule;
use crate::lint::ar;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::{ElfFacts, entry_bytes};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct NoCodeSections;

declare_rule! {
    /// A static library (`.a` archive) has no member with usable code: none
    /// of its ELF members has a non-empty `.text` or `.data` section, or a
    /// non-empty `.preinit_array`, `.init_array` or `.fini_array`. It happens
    /// when shared objects are built with `-flto=auto` but without
    /// `-ffat-lto-objects`, and `dh_strip` strips the LTO sections, leaving
    /// nothing to link against.
    ///
    /// Walks the Data files (regular files and hardlinks) that have the `ar`
    /// magic, skipping paths under `usr/lib/ghc/` as lintian does. Members are
    /// read with the shared archive parser and each ELF member's sections
    /// with their sizes ([`ElfFacts::sized_section_names`]). A member that is
    /// not ELF has no code. Extra is empty; the archive path is the Location.
    /// Not skipped for udebs, like lintian. `file(1)`'s "current ar archive"
    /// test is replaced by the `ar` magic.
    NoCodeSections,
    code = "LN1416",
    tag = "no-code-sections",
    default_selected = true,
    experimental = false,
}

const ARRAY_SECTIONS: [&str; 3] = [".preinit_array", ".init_array", ".fini_array"];

impl BinaryPackageRule for NoCodeSections {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in archives_without_code(files) {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// The paths of the static libraries in `files` with no member that has code,
/// in path order.
fn archives_without_code(files: &FileTree) -> Vec<std::path::PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter(|entry| !entry.path().starts_with("usr/lib/ghc"))
        .filter_map(|entry| {
            let bytes = entry_bytes(files, entry)?;
            if !ar::is_archive(&bytes) {
                return None;
            }
            let has_code = ar::members(&bytes).iter().any(|member| {
                ElfFacts::from_bytes(member.data).is_some_and(|facts| member_has_code(&facts))
            });
            (!has_code).then(|| entry.path().to_path_buf())
        })
        .collect()
}

fn member_has_code(facts: &ElfFacts) -> bool {
    facts.sized_section_names().any(|name| {
        name.starts_with(".text") || name.starts_with(".data") || ARRAY_SECTIONS.contains(&name)
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    /// An `ar` archive whose members are `(name, data)`.
    fn archive(members: &[(&str, Vec<u8>)]) -> TestFile {
        let mut bytes = b"!<arch>\n".to_vec();
        for (name, data) in members {
            bytes.extend(
                format!(
                    "{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
                    format!("{name}/"),
                    0,
                    0,
                    0,
                    644,
                    data.len()
                )
                .into_bytes(),
            );
            bytes.extend_from_slice(data);
            if data.len() % 2 == 1 {
                bytes.push(b'\n');
            }
        }
        TestFile::from_bytes(bytes)
    }

    fn elf(elf: TestElf) -> Vec<u8> {
        elf.build()
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[("foo.o", elf(TestElf::executable().sized_section(".text")))]),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[(
                    "foo.o",
                    elf(TestElf::executable().sized_section(".data.rel.ro")),
                )]),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[(
                    "foo.o",
                    elf(TestElf::executable().sized_section(".init_array")),
                )]),
            )]),
            // One codeful member is enough.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[
                    ("empty.o", elf(TestElf::executable().section(".text"))),
                    ("foo.o", elf(TestElf::executable().sized_section(".text"))),
                ]),
            )]),
            // Under usr/lib/ghc the Check does not look.
            BinaryPackageCase::from(vec![(
                "usr/lib/ghc/lib/libfoo.a",
                archive(&[("foo.o", elf(TestElf::executable()))]),
            )]),
            // Not an archive.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.a", TestFile::new("text\n"))]),
        ];
        let fail = vec![
            // Only data without contents.
            BinaryPackageCase::from(vec![(
                "usr/lib/libcodeless.a",
                archive(&[("foo.o", elf(TestElf::executable().section(".text")))]),
            )]),
            // A non-code section is not code.
            BinaryPackageCase::from(vec![(
                "usr/lib/libcodeless.a",
                archive(&[("foo.o", elf(TestElf::executable().sized_section(".note")))]),
            )]),
            // No members, and members that are not ELF.
            BinaryPackageCase::from(vec![("usr/lib/empty.a", archive(&[]))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/text.a",
                archive(&[("notes.txt", b"text".to_vec())]),
            )]),
            // A hardlink counts as a file, like lintian's `is_file`.
            BinaryPackageCase::from(vec![(
                "usr/lib/libcodeless-link.a",
                archive(&[("foo.o", elf(TestElf::executable()))]).hardlink(),
            )]),
            // udebs are not skipped.
            BinaryPackageCase::from(vec![(
                "usr/lib/libcodeless.a",
                archive(&[("foo.o", elf(TestElf::executable()))]),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(NoCodeSections, pass, fail).test_and_snapshot();
    }
}
