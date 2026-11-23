use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct StaticLinkTimeOptimization;

declare_rule! {
    /// A member of a static library (`.a` archive) ships ELF sections named
    /// `.gnu.lto*`, which come from building with `-flto`: the library works
    /// but is larger than needed, and usually a bug. Extra is the member's
    /// name as `ar t` prints it (`code.o`); the archive's path is the Location.
    /// One hint per such member, in archive order.
    ///
    /// Walks the archives of the Data files (regular files and hardlinks)
    /// through the shared ELF members of each archive
    /// ([`ElfArchiveMembers`](crate::lint::elf::ElfArchiveMembers)), skipping
    /// paths under `usr/lib/ghc/` as lintian does. Any section name counts,
    /// sized or not. Severity is info, and the Tag is not selected by default.
    StaticLinkTimeOptimization,
    code = "LN1574",
    tag = "static-link-time-optimization",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for StaticLinkTimeOptimization {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(archives) = ctx.data_elf_members() else {
            return;
        };
        for (path, members) in archives.iter() {
            if path.starts_with("usr/lib/ghc") {
                continue;
            }
            for member in members {
                let has_lto = member
                    .facts()
                    .section_names()
                    .iter()
                    .any(|name| name.starts_with(".gnu.lto"));
                if !has_lto {
                    continue;
                }
                ctx.diagnostic(
                    Diagnostic::warn(member.name())
                        .with_severity(Severity::Info)
                        .with_location(Location {
                            path: path.to_path_buf(),
                            line: None,
                            column: None,
                        }),
                );
            }
        }
    }
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

    #[test]
    fn binary() {
        let pass = vec![
            // Ordinary code sections.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[(
                    "foo.o",
                    TestElf::executable().sized_section(".text").build(),
                )]),
            )]),
            // Under usr/lib/ghc the Check does not look.
            BinaryPackageCase::from(vec![(
                "usr/lib/ghc/lib/libfoo.a",
                archive(&[(
                    "foo.o",
                    TestElf::executable().section(".gnu.lto_.text.1").build(),
                )]),
            )]),
            // Not an archive, and an archive without ELF members.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                TestFile::new("!<arch>\nnot an archive\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[("notes.txt", b"text".to_vec())]),
            )]),
        ];
        let fail = vec![
            // Any section name counts, sized or not.
            BinaryPackageCase::from(vec![(
                "usr/lib/libcodeless.a",
                archive(&[(
                    "code.o",
                    TestElf::executable().section(".gnu.lto_.text.1").build(),
                )]),
            )]),
            // Only the members with LTO sections are hinted.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[
                    (
                        "plain.o",
                        TestElf::executable().sized_section(".text").build(),
                    ),
                    (
                        "lto.o",
                        TestElf::executable()
                            .sized_section(".text")
                            .section(".gnu.lto.symtab")
                            .build(),
                    ),
                ]),
            )]),
            // A hardlink counts as a file.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo-link.a",
                archive(&[(
                    "code.o",
                    TestElf::executable().section(".gnu.lto_.text.1").build(),
                )])
                .hardlink(),
            )]),
        ];
        BinaryPackageTester::new(StaticLinkTimeOptimization, pass, fail).test_and_snapshot();
    }
}
