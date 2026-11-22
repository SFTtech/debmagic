use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::shared::shared_libraries;

pub struct LdconfigSymlinkIsNotASymlink;

declare_rule! {
    /// The versioned link of a shared library's SONAME is installed, but it is
    /// not a symbolic link. Reported at the link with extra `should point to
    /// <library>`, the library's path.
    ///
    /// The link is looked up in the same folder; a library whose SONAME is its
    /// own name is not checked. Udebs are checked too.
    LdconfigSymlinkIsNotASymlink,
    code = "LN1411",
    tag = "ldconfig-symlink-is-not-a-symlink",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for LdconfigSymlinkIsNotASymlink {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf) = ctx.data_elf_files() else {
            return;
        };
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut findings: Vec<(PathBuf, String)> = Vec::new();
        for (library, versioned) in shared_libraries(files, &elf) {
            if versioned == library.to_string_lossy() {
                continue;
            }
            let Some(link) = files.get(&versioned) else {
                continue;
            };
            if !link.is_symlink() {
                findings.push((
                    link.path().to_path_buf(),
                    format!("should point to {}", library.display()),
                ));
            }
        }
        for (path, extra) in findings {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
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
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", elf(library()))]),
            // No link at all is another Rule's finding.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1.0.1", elf(library()))]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo.so.1.0.1", elf(library())),
                ("usr/lib/libfoo.so.1", TestFile::new("copy")),
            ]),
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo.so.1.0.1", elf(library())),
                ("usr/lib/libfoo.so.1", TestFile::directory()),
            ])
            .udeb(),
        ];
        BinaryPackageTester::new(LdconfigSymlinkIsNotASymlink, pass, fail).test_and_snapshot();
    }
}
