use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct UnstrippedStaticLibrary;

declare_rule! {
    /// A static library has members with debugging information.
    ///
    /// A member with a `.debug_info` or `.zdebug_info` section was not
    /// stripped with `strip --strip-debug`. Walks the ELF members of each
    /// archive of the Data files (regular files and hardlinks). An archive
    /// whose name ends in `_g.a` is a debug library and is skipped. Reported
    /// once per archive, with Extra `(<member> <member>)`, the member names
    /// sorted and unique and separated by spaces; the archive path is the
    /// Location. Udebs are checked too.
    UnstrippedStaticLibrary,
    code = "LN1752",
    tag = "unstripped-static-library",
    default_selected = false,
    experimental = false,
}

const DEBUG: [&str; 2] = [".debug_info", ".zdebug_info"];

impl BinaryPackageRule for UnstrippedStaticLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(archives) = ctx.data_elf_members() else {
            return;
        };
        for (path, members) in archives.iter() {
            if path.to_string_lossy().ends_with("_g.a") {
                continue;
            }
            let mut unstripped: Vec<&str> = members
                .iter()
                .filter(|member| {
                    member
                        .facts()
                        .section_names()
                        .iter()
                        .any(|name| DEBUG.contains(&name.as_str()))
                })
                .map(|member| member.name())
                .collect();
            if unstripped.is_empty() {
                continue;
            }
            unstripped.sort_unstable();
            unstripped.dedup();
            ctx.diagnostic(
                Diagnostic::warn(format!("({})", unstripped.join(" ")))
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
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[(
                    "foo.o",
                    TestElf::executable().sized_section(".text").build(),
                )]),
            )]),
            // A stripped debug section list (only .comment) is another Tag's.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[("foo.o", TestElf::executable().section(".comment").build())]),
            )]),
            // A debug library is skipped.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo_g.a",
                archive(&[(
                    "foo.o",
                    TestElf::executable().section(".debug_info").build(),
                )]),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                TestFile::new("!<arch>\nnot an archive\n"),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[(
                    "code.o",
                    TestElf::executable().sized_section(".debug_info").build(),
                )]),
            )]),
            // The unstripped members, sorted and unique.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[
                    ("b.o", TestElf::executable().section(".zdebug_info").build()),
                    ("a.o", TestElf::executable().section(".debug_info").build()),
                    ("c.o", TestElf::executable().sized_section(".text").build()),
                ]),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                archive(&[(
                    "code.o",
                    TestElf::executable().section(".debug_info").build(),
                )]),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(UnstrippedStaticLibrary, pass, fail).test_and_snapshot();
    }
}
