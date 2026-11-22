use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::ElfFiles;
use crate::lint::rule::BinaryPackageRule;

pub struct ElfError;

declare_rule! {
    /// `readelf` could not parse part of an ELF object. Lintian quotes the
    /// error line, prefixed with the `readelf` paragraph it came from, as the
    /// extra: `In ELF header: Section headers are not available!`.
    ///
    /// Walks the ELF facts of Data files (regular files and hardlinks) and
    /// reports each distinct error of [`ElfFacts::errors`] once, in the order
    /// `readelf` prints it. The missing interpreter of a split-off debug file
    /// (`PT_INTERP` with no contents) is not an error under `usr/lib/debug/`. Extra is the error line; the Data files path
    /// is the Location. Not skipped for udebs, like lintian.
    ///
    /// Only the errors this module can see are reported (a Section header
    /// table past the end of the file, a `PT_INTERP` outside the file); other
    /// `readelf` errors and all `readelf` warnings (`elf-warning`) are not.
    /// lintian's per-member errors of static libraries and its Source tree
    /// (`visit_patched_files`) pass are not covered.
    ElfError,
    code = "LN1292",
    tag = "elf-error",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ElfError {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for (path, error) in errors(&elf_files) {
            ctx.diagnostic(Diagnostic::warn(error).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

const DEBUG_INTERPRETER_ERROR: &str = "In program headers: Unable to find program interpreter name";

/// The `(path, error)` hits, in path order, then in `readelf` order.
fn errors(elf_files: &ElfFiles) -> Vec<(std::path::PathBuf, String)> {
    let mut hits = Vec::new();
    for (path, facts) in elf_files.iter() {
        let in_debug_tree = path.starts_with("usr/lib/debug");
        let mut seen: Vec<&str> = Vec::new();
        for error in facts.errors() {
            if seen.contains(&error.as_str()) {
                continue;
            }
            seen.push(error);
            if in_debug_tree && error == DEBUG_INTERPRETER_ERROR {
                continue;
            }
            hits.push((path.to_path_buf(), error.clone()));
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    /// A PT_INTERP with no contents, as in a split-off debug file made by
    /// `objcopy --only-keep-debug`.
    fn interp_without_contents() -> TestFile {
        let mut bytes = TestElf::executable()
            .interpreter("/lib64/ld-linux-x86-64.so.2")
            .build();
        bytes[64 + 56 + 32..64 + 56 + 40].copy_from_slice(&0u64.to_le_bytes());
        TestFile::from_bytes(bytes)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("usr/bin/hello", elf(TestElf::executable()))]),
            // A complete section table is not an error.
            BinaryPackageCase::from(vec![(
                "usr/bin/hello",
                elf(TestElf::executable().import("puts", None)),
            )]),
            // Not an ELF object.
            BinaryPackageCase::from(vec![(
                "usr/bin/hello.txt",
                TestFile::new("Section headers are not available!\n"),
            )]),
            // The interpreter of a split-off debug file is expected to be missing.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/bin/hello",
                interp_without_contents(),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/bin/elfobject",
                elf(TestElf::executable().missing_section_headers()),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/foo/fake-guile-object.go",
                elf(TestElf::shared_object().missing_section_headers()),
            )]),
            BinaryPackageCase::from(vec![("usr/bin/hello.dbg", interp_without_contents())]),
            // A hardlink counts as a file, like lintian's `is_file`.
            BinaryPackageCase::from(vec![(
                "usr/bin/elfobject-link",
                elf(TestElf::executable().missing_section_headers()).hardlink(),
            )]),
            // udebs are not skipped.
            BinaryPackageCase::from(vec![(
                "usr/bin/elfobject",
                elf(TestElf::executable().missing_section_headers()),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(ElfError, pass, fail).test_and_snapshot();
    }
}
