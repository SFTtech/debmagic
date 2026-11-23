use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::ElfObjectType;
use crate::lint::rule::BinaryPackageRule;

pub struct OcamlCustomExecutable;

declare_rule! {
    /// An OCaml byte-code executable linked with a custom runtime: an ELF
    /// executable (`file(1)`: "executable", so `ET_EXEC` or a PIE, not a
    /// shared object) that is "not stripped" (it has a `.symtab` section) and
    /// whose strings include the `Caml1999X0` magic followed by two digits, on
    /// a line of its own. Such executables cannot be stripped and are
    /// deprecated in favour of shared libraries for C stubs.
    ///
    /// Walks the ELF Data files (regular files and hardlinks, as lintian's
    /// `is_file`); the strings are the shared `FileStrings`, which skip
    /// `usr/lib/debug/`. Extra is empty; the executable's path is the Location.
    OcamlCustomExecutable,
    code = "LN1401",
    tag = "ocaml-custom-executable",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for OcamlCustomExecutable {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        let Some(strings) = ctx.data_strings() else {
            return;
        };
        for (path, facts) in elf_files.iter() {
            if facts.object_type() != ElfObjectType::Executable || !facts.has_symbol_table() {
                continue;
            }
            if !strings.get(path).is_some_and(has_caml_magic) {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: path.to_path_buf(),
                line: None,
                column: None,
            }));
        }
    }
}

/// A line of the strings that is `Caml1999X0` and two digits, as lintian's
/// `^ Caml1999X0 [0-9] [0-9] $` matches it on one line.
fn has_caml_magic(strings: &str) -> bool {
    strings.lines().any(|line| {
        line.strip_prefix("Caml1999X0").is_some_and(|digits| {
            digits.len() == 2 && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    /// An ELF whose bytes end with the OCaml magic line, as a custom runtime
    /// executable carries it in its data.
    fn ocaml_elf(elf: TestElf) -> TestFile {
        ocaml_text(elf, format!("\0{}\0", caml_magic("08")).as_bytes())
    }

    /// The OCaml magic with `digits`. Built at run time: the test binary is
    /// itself linted by other tests, and a literal would look like bytecode.
    fn caml_magic(digits: &str) -> String {
        ["Caml1999X0", digits].concat()
    }

    /// An ELF with `tail` appended to its bytes.
    fn ocaml_text(elf: TestElf, tail: &[u8]) -> TestFile {
        let mut bytes = elf.build();
        bytes.extend_from_slice(tail);
        TestFile::from_bytes(bytes)
    }

    #[test]
    fn binary() {
        let pass = vec![
            // No OCaml magic.
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/ocaml-exec",
                TestFile::from_bytes(TestElf::executable().build()),
            )]),
            // Stripped (no symbol table): file(1) says "stripped".
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/ocaml-exec",
                ocaml_elf(TestElf::executable()),
            )]),
            // A shared object is not an executable.
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/ocaml-exec",
                ocaml_elf(TestElf::shared_object().section(".symtab")),
            )]),
            // Not ELF.
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/ocaml-exec",
                TestFile::new(format!("{}\n", caml_magic("08"))),
            )]),
            // The magic must be a line of its own, with two digits.
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/ocaml-exec",
                ocaml_text(
                    TestElf::executable(),
                    format!("\0x {}\0{}0\0", caml_magic("08"), caml_magic("08")).as_bytes(),
                ),
            )]),
            // Under usr/lib/debug the strings are not read.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/foo/ocaml-exec",
                ocaml_elf(TestElf::executable().section(".symtab")),
            )]),
        ];
        let fail = vec![
            // "not stripped": the symbol table is there.
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/ocaml-exec",
                ocaml_elf(TestElf::executable().section(".symtab")),
            )]),
            // A PIE executable is "pie executable" to file(1).
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/ocaml-exec",
                ocaml_elf(TestElf::pie_executable().section(".symtab")),
            )]),
            // A hardlink counts as a file.
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/ocaml-exec",
                ocaml_elf(TestElf::executable().section(".symtab")).hardlink(),
            )]),
        ];
        BinaryPackageTester::new(OcamlCustomExecutable, pass, fail).test_and_snapshot();
    }
}
