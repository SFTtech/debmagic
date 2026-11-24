use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct OddPermissionsOnSharedLibrary;

declare_rule! {
    /// A shared library is neither executable nor mode `0644`.
    ///
    /// A shared library is a regular file or hardlink whose ELF facts have a
    /// `DT_SONAME`. It is reported when it has no execute bit and its
    /// permission bits (`0o7777`, setuid and the like included) are not
    /// `0644`. Reported with the octal mode (four digits, `0600`) as Extra and
    /// the path as Location. A shared library with an execute bit is left to
    /// `shared-library-is-executable`. udebs are not skipped.
    OddPermissionsOnSharedLibrary,
    code = "LN1125",
    tag = "odd-permissions-on-shared-library",
    default_selected = true,
    experimental = false,
}

/// Permission bits lintian calls widely readable: `oct(644)`.
const WIDELY_READABLE: u32 = 0o644;

impl BinaryPackageRule for OddPermissionsOnSharedLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(PathBuf, u32)> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_file())
            .filter(|entry| {
                elf_files
                    .get(entry.path())
                    .is_some_and(|facts| !facts.soname().is_empty())
            })
            .filter(|entry| !entry.is_executable_file())
            .filter(|entry| entry.mode() != WIDELY_READABLE)
            .map(|entry| (entry.path().to_path_buf(), entry.mode()))
            .collect();
        for (path, mode) in hits {
            ctx.diagnostic(
                Diagnostic::warn(format!("{mode:04o}")).with_location(Location {
                    path,
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

    fn shared_object(mode: u32) -> TestFile {
        TestFile::from_bytes(TestElf::shared_object().soname("libfoo.so.1").build()).mode(mode)
    }

    #[test]
    fn binary() {
        let pass = vec![
            // The usual mode.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", shared_object(0o644))]),
            // Executable: left to shared-library-is-executable.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", shared_object(0o755))]),
            // Not a shared library: no SONAME.
            BinaryPackageCase::from(vec![(
                "usr/bin/foo",
                TestFile::from_bytes(TestElf::executable().needed("libc.so.6").build()).mode(0o600),
            )]),
            // Not ELF at all.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                TestFile::new("text").mode(0o600),
            )]),
            // A symlink is not a file.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so",
                TestFile::symlink("libfoo.so.1"),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", shared_object(0o600))]),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", shared_object(0o664))]),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", shared_object(0o640))]),
            // A hardlink is a file.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.2",
                shared_object(0o600).hardlink(),
            )]),
            // A udeb is not skipped.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", shared_object(0o600))]).udeb(),
        ];
        BinaryPackageTester::new(OddPermissionsOnSharedLibrary, pass, fail).test_and_snapshot();
    }
}
