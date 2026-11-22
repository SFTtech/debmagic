use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use std::path::Path;

use super::shared::shared_libraries;

pub struct LdconfigSymlinkReferencingWrongFile;

/// The last component of a path as text, for messages.
pub(super) fn basename(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

declare_rule! {
    /// The versioned link of a shared library's SONAME is a symbolic link to
    /// something other than the library's file name. Reported at the link with
    /// extra `should point to <target> instead of <library basename>`.
    ///
    /// A library whose SONAME is its own name is not checked. Udebs are
    /// checked too.
    LdconfigSymlinkReferencingWrongFile,
    code = "LN1412",
    tag = "ldconfig-symlink-referencing-wrong-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for LdconfigSymlinkReferencingWrongFile {
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
            let Some(target) = link.link_target() else {
                continue;
            };
            let name = basename(&library);
            if target.as_os_str().as_encoded_bytes() != name.as_bytes() {
                findings.push((
                    link.path().to_path_buf(),
                    format!(
                        "should point to {} instead of {name}",
                        target.to_string_lossy()
                    ),
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
            // A regular file is another Rule's finding.
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo.so.1.0.1", elf(library())),
                ("usr/lib/libfoo.so.1", TestFile::new("x")),
            ]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo.so.1.0.1", elf(library())),
                ("usr/lib/libfoo.so.1", TestFile::symlink("some-other-file")),
            ]),
            BinaryPackageCase::from(vec![
                ("usr/lib/x86_64-linux-gnu/libfoo.so.1.0.1", elf(library())),
                (
                    "usr/lib/x86_64-linux-gnu/libfoo.so.1",
                    TestFile::symlink("libfoo.so.1.0"),
                ),
            ])
            .udeb(),
        ];
        BinaryPackageTester::new(LdconfigSymlinkReferencingWrongFile, pass, fail)
            .test_and_snapshot();
    }
}
