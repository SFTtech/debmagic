use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct OddStaticLibraryName;

declare_rule! {
    /// A static archive is installed as `<name>_s.a` while `<name>.a` is not
    /// installed by the same package.
    ///
    /// Walks Data files that are regular files or hardlinks, whose name ends
    /// with `_s.a`, and that are a static archive (`!<arch>` magic that is not
    /// a `.deb`). Reported when no non-directory entry named with the `_s.a`
    /// suffix replaced by `.a` exists in the package. Reported at the `_s.a`
    /// path with an empty extra. Udebs are checked too.
    OddStaticLibraryName,
    code = "LN0768",
    tag = "odd-static-library-name",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for OddStaticLibraryName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter_map(|entry| {
                let name = entry.path().to_str()?;
                let shortened = format!("{}.a", name.strip_suffix("_s.a")?);
                let installed = files
                    .get(&shortened)
                    .is_some_and(|other| other.kind() != EntryKind::Directory);
                let archive = files
                    .read(entry)
                    .is_ok_and(|bytes| is_static_archive(&bytes));
                (archive && !installed).then(|| entry.path().to_path_buf())
            })
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Same test as `ln0722`'s `is_static_archive`: GNU `ar` magic, not a `.deb`.
fn is_static_archive(bytes: &[u8]) -> bool {
    bytes
        .strip_prefix(b"!<arch>\n")
        .is_some_and(|members| !members.starts_with(b"debian-binary"))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn archive() -> TestFile {
        TestFile::from_bytes(
            b"!<arch>\nlibfoo.o/      0           0     0     644     4         `\nabcd".to_vec(),
        )
    }

    #[test]
    fn archive_detection() {
        assert!(is_static_archive(b"!<arch>\nlibfoo.o/"));
        assert!(!is_static_archive(b"!<arch>text"));
        assert!(!is_static_archive(b"!<arch>\ndebian-binary   "));
        assert!(!is_static_archive(b"text"));
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // The `.a` name is installed alongside.
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo_s.a", archive()),
                ("usr/lib/libfoo.a", archive()),
            ])
            .field("Architecture", "amd64"),
            // A symbolic link of the short name counts as installed.
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo_s.a", archive()),
                ("usr/lib/libfoo.a", TestFile::symlink("libfoo.so")),
            ])
            .field("Architecture", "amd64"),
            // Not a static archive.
            BinaryPackageCase::from(vec![("usr/lib/libfoo_s.a", TestFile::new("text\n"))])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo_s.a",
                TestFile::from_bytes(b"!<arch>\ndebian-binary   ".to_vec()),
            )])
            .field("Architecture", "amd64"),
            // Only the `_s.a` suffix is judged.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.a", archive())])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo_s.so", archive())])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo_s.a.in", archive())])
                .field("Architecture", "amd64"),
            // Symbolic links and directories are not regular files.
            BinaryPackageCase::from(vec![("usr/lib/libfoo_s.a", TestFile::symlink("libfoo.a"))])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo_s.a", TestFile::directory())])
                .field("Architecture", "amd64"),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/lib/libfoo_s.a", archive())])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/sub/libfoo_s.a", archive())])
                .field("Architecture", "amd64"),
            // A directory of the short name is not an installed file.
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo_s.a", archive()),
                ("usr/lib/libfoo.a", TestFile::directory()),
            ])
            .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo_s.a", archive().hardlink())])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo_s.a", archive())])
                .field("Architecture", "all")
                .udeb(),
        ];
        BinaryPackageTester::new(OddStaticLibraryName, pass, fail).test_and_snapshot();
    }
}
