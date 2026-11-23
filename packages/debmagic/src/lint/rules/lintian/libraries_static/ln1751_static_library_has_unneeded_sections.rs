use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct StaticLibraryHasUnneededSections;

declare_rule! {
    /// A member of a static library still has a `.note` or `.comment` section.
    ///
    /// Those sections are not useful in a static library. `strip
    /// --remove-section=.comment --remove-section=.note` removes them, which
    /// `dh_strip` does and `install -s` does not. Walks the ELF members of each
    /// archive of the Data files (regular files and hardlinks). A member with a
    /// `.debug_info` or `.zdebug_info` section is left to
    /// `unstripped-static-library` and is not checked here. An archive whose
    /// name ends in `_g.a` is a debug library and is skipped. Reported as an
    /// info hint per member with such a section, with Extra
    /// `(<member>) <sections>`, the sections sorted and unique and separated by
    /// spaces; the archive path is the Location. Udebs are checked too.
    StaticLibraryHasUnneededSections,
    code = "LN1751",
    tag = "static-library-has-unneeded-sections",
    default_selected = false,
    experimental = false,
}

const UNNEEDED: [&str; 2] = [".note", ".comment"];
const DEBUG: [&str; 2] = [".debug_info", ".zdebug_info"];

impl BinaryPackageRule for StaticLibraryHasUnneededSections {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(archives) = ctx.data_elf_members() else {
            return;
        };
        for (path, members) in archives.iter() {
            if path.to_string_lossy().ends_with("_g.a") {
                continue;
            }
            for member in members {
                let names = member.facts().section_names();
                if names.iter().any(|name| DEBUG.contains(&name.as_str())) {
                    continue;
                }
                let mut sections: Vec<&str> = UNNEEDED
                    .into_iter()
                    .filter(|wanted| names.iter().any(|name| name == wanted))
                    .collect();
                if sections.is_empty() {
                    continue;
                }
                sections.sort_unstable();
                ctx.diagnostic(
                    Diagnostic::warn(format!("({}) {}", member.name(), sections.join(" ")))
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
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Only code sections.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[(
                    "foo.o",
                    TestElf::executable().sized_section(".text").build(),
                )]),
            )]),
            // A debug member is left to the unstripped Tag.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[(
                    "foo.o",
                    TestElf::executable()
                        .section(".debug_info")
                        .section(".comment")
                        .build(),
                )]),
            )]),
            // A debug library is skipped.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo_g.a",
                archive(&[("foo.o", TestElf::executable().section(".comment").build())]),
            )]),
            // Not an archive.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                TestFile::new("!<arch>\nnot an archive\n"),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[(
                    "pstripped.o",
                    TestElf::executable()
                        .sized_section(".text")
                        .section(".comment")
                        .build(),
                )]),
            )]),
            // Both sections, sorted, and only the members that have them.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[
                    (
                        "plain.o",
                        TestElf::executable().sized_section(".text").build(),
                    ),
                    (
                        "both.o",
                        TestElf::executable()
                            .section(".note")
                            .section(".comment")
                            .build(),
                    ),
                ]),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[(
                    "pstripped.o",
                    TestElf::executable().section(".comment").build(),
                )]),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(StaticLibraryHasUnneededSections, pass, fail).test_and_snapshot();
    }
}
