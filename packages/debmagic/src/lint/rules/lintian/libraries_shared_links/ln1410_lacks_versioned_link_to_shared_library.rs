use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::shared::shared_libraries;

pub struct LacksVersionedLinkToSharedLibrary;

declare_rule! {
    /// A shared library in an ldconfig folder has no versioned link of its
    /// SONAME in the same folder. Reported at the library with extra
    /// `<directory>/<SONAME>`.
    ///
    /// A regular file or hardlink counts as a shared library when its dynamic
    /// section has a SONAME. The link itself may be any kind of entry. Udebs
    /// are checked too.
    LacksVersionedLinkToSharedLibrary,
    code = "LN1410",
    tag = "lacks-versioned-link-to-shared-library",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for LacksVersionedLinkToSharedLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf) = ctx.data_elf_files() else {
            return;
        };
        let Some(files) = ctx.data_files() else {
            return;
        };
        let missing: Vec<(PathBuf, String)> = shared_libraries(files, &elf)
            .into_iter()
            .filter(|(_, versioned)| files.get(versioned).is_none())
            .collect();
        for (path, versioned) in missing {
            ctx.diagnostic(Diagnostic::error(versioned).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    fn library() -> TestElf {
        TestElf::shared_object().soname("libfoo.so.1")
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo.so.1.0.1", elf(library())),
                ("usr/lib/libfoo.so.1", TestFile::symlink("libfoo.so.1.0.1")),
            ]),
            // The library is its own versioned name.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", elf(library()))]),
            // Any kind of entry counts as the link.
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo.so.1.0.1", elf(library())),
                ("usr/lib/libfoo.so.1", TestFile::directory()),
            ]),
            // Not in an ldconfig folder.
            BinaryPackageCase::from(vec![("usr/share/libfoo.so.1.0.1", elf(library()))]),
            // No SONAME.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1.0.1",
                elf(TestElf::shared_object()),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1.0.1", elf(library()))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/libfoo.so.1.0.1",
                elf(library()),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(LacksVersionedLinkToSharedLibrary, pass, fail).test_and_snapshot();
    }
}
