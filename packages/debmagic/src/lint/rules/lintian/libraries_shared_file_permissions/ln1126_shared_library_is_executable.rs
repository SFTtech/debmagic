use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct SharedLibraryIsExecutable;

declare_rule! {
    /// A shared library installed with an execute bit set.
    ///
    /// Walks Data files regular files and hardlinks that are executable and
    /// whose ELF dynamic section has a `SONAME`. Reported with the extra
    /// `<mode>` (`0755`, the permission bits in octal); the path is the
    /// Location. Skipped, as lintian does: an ELF with a `PT_INTERP`
    /// interpreter (it runs by design), and the dynamic linker itself, a path
    /// starting with `lib` whose file name is `ld-<digits and dots>.so`.
    /// Udebs are checked too. The sibling Tag `odd-permissions-on-shared-library`
    /// (non-executable, not `0644`) is a separate Rule.
    SharedLibraryIsExecutable,
    code = "LN1126",
    tag = "shared-library-is-executable",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SharedLibraryIsExecutable {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let executables: Vec<(PathBuf, u32)> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_executable_file())
            .map(|entry| (entry.path().to_path_buf(), entry.mode()))
            .collect();
        if executables.is_empty() {
            return;
        }
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        let hits: Vec<(PathBuf, u32)> = executables
            .into_iter()
            .filter(|(path, _)| {
                elf_files.get(path).is_some_and(|facts| {
                    !facts.soname().is_empty() && facts.interpreter().is_none()
                }) && !is_dynamic_linker(path)
            })
            .collect();
        for (path, mode) in hits {
            ctx.diagnostic(
                Diagnostic::error(format!("{mode:04o}")).with_location(Location {
                    path,
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

/// Lintian's `^lib.*/ld-[\d.]+\.so$` on the raw path bytes: the name starts
/// with `lib`, and the last component is `ld-` plus digits and dots plus `.so`
/// after a `/`.
fn is_dynamic_linker(path: &std::path::Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    let bytes = path.as_os_str().as_bytes();
    let Some(rest) = bytes.strip_prefix(b"lib") else {
        return false;
    };
    let Some(slash) = rest.iter().rposition(|&byte| byte == b'/') else {
        return false;
    };
    let Some(version) = rest[slash + 1..]
        .strip_prefix(b"ld-")
        .and_then(|name| name.strip_suffix(b".so"))
    else {
        return false;
    };
    !version.is_empty()
        && version
            .iter()
            .all(|&byte| byte.is_ascii_digit() || byte == b'.')
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    fn shared(mode: u32) -> TestFile {
        elf(TestElf::shared_object().soname("libfoo.so.1")).mode(mode)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1.0.1", shared(0o644))]),
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                elf(TestElf::executable().needed("libc.so.6")).mode(0o755),
            )]),
            // Shared object with an interpreter runs by design.
            BinaryPackageCase::from(vec![(
                "usr/lib/librun.so.1",
                elf(TestElf::pie_executable()
                    .soname("librun.so.1")
                    .interpreter("/lib64/ld-linux-x86-64.so.2"))
                .mode(0o755),
            )]),
            // The dynamic linker is exempt.
            BinaryPackageCase::from(vec![("lib/x86_64-linux-gnu/ld-2.31.so", shared(0o755))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/script",
                TestFile::new("#!/bin/sh\n").mode(0o755),
            )]),
            BinaryPackageCase::from(vec![("usr/lib/link", TestFile::symlink("libfoo.so.1"))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.a",
                TestFile::new("!<arch>\n").mode(0o755),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1.0.1", shared(0o755))]),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1.0.1", shared(0o744))]),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1.0.1", shared(0o4755))]),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1.0.1", shared(0o755).hardlink())]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1.0.1",
                shared(0o755).hardlink_to("usr/lib/libfoo.so.1.0.0"),
            )]),
            // The dynamic linker exemption needs a `lib...` path.
            BinaryPackageCase::from(vec![("usr/lib/ld-2.31.so", shared(0o755))]),
            // A dynamic linker not named like `ld-<version>.so`.
            BinaryPackageCase::from(vec![("lib/ld-linux.so.2", shared(0o755))]),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1.0.1", shared(0o755))]).udeb(),
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/lib/lib\xff.so.1".to_vec()),
                shared(0o755),
            )]),
        ];
        BinaryPackageTester::new(SharedLibraryIsExecutable, pass, fail).test_and_snapshot();
    }
}
