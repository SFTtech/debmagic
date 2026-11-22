use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::shared::shared_libraries;

pub struct LinkToSharedLibraryInWrongPackage;

/// lintian's `$unversioned_name`: `-<digits and dots>.so` becomes `.so`, then
/// everything after the first `.so` that is followed by more text is dropped.
pub(super) fn unversioned_name(versioned: &str) -> String {
    let mut name = versioned.to_string();
    if let Some(stem) = name.strip_suffix(".so") {
        let trimmed = stem.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        if trimmed.len() < stem.len() && trimmed.ends_with('-') {
            name = format!("{}.so", &trimmed[..trimmed.len() - 1]);
        }
    }
    if let Some(position) = name
        .match_indices(".so")
        .map(|(position, _)| position)
        .find(|position| position + 3 < name.len())
    {
        name.truncate(position + 3);
    }
    name
}

declare_rule! {
    /// A shared library's unversioned `.so` link is installed in the same
    /// package as the library. The link belongs in a `-dev` package. Reported
    /// at the link with extra the library's path.
    ///
    /// The unversioned name is derived from the library's path: its
    /// `-<version>.so` form and everything after the first `.so`. A library
    /// without a version in its name is not checked. Udebs are checked too.
    LinkToSharedLibraryInWrongPackage,
    code = "LN1413",
    tag = "link-to-shared-library-in-wrong-package",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for LinkToSharedLibraryInWrongPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf) = ctx.data_elf_files() else {
            return;
        };
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut findings: Vec<(PathBuf, String)> = Vec::new();
        for (library, versioned) in shared_libraries(files, &elf) {
            let unversioned = unversioned_name(&versioned);
            if unversioned == library.to_string_lossy() {
                continue;
            }
            if let Some(link) = files.get(&unversioned) {
                findings.push((
                    link.path().to_path_buf(),
                    library.to_string_lossy().into_owned(),
                ));
            }
        }
        for (path, extra) in findings {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
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

    #[test]
    fn unversioned_name_drops_version_and_suffix() {
        assert_eq!(
            unversioned_name("usr/lib/libfoo.so.1.0.1"),
            "usr/lib/libfoo.so"
        );
        assert_eq!(
            unversioned_name("usr/lib/libfoo-1.2.so"),
            "usr/lib/libfoo.so"
        );
        assert_eq!(unversioned_name("usr/lib/libfoo.so"), "usr/lib/libfoo.so");
    }

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    fn library() -> TestElf {
        TestElf::shared_object().soname("libfoo.so.1")
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1.0.1", elf(library()))]),
            // The unversioned link is in another package: nothing to report here.
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo.so.1.0.1", elf(library())),
                ("usr/lib/libbar.so", TestFile::new("x")),
            ]),
            // A library whose name has no version has no unversioned link to find.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so",
                elf(TestElf::shared_object().soname("libfoo.so")),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo.so.1.0.1", elf(library())),
                ("usr/lib/libfoo.so", TestFile::symlink("libfoo.so.1.0.1")),
            ]),
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo.so.1.0.1", elf(library())),
                ("usr/lib/libfoo.so", TestFile::new("x")),
            ])
            .udeb(),
        ];
        BinaryPackageTester::new(LinkToSharedLibraryInWrongPackage, pass, fail).test_and_snapshot();
    }
}
