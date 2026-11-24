use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_type::FileType;
use crate::lint::rule::BinaryPackageRule;

pub struct StrippedLibrary;

declare_rule! {
    /// A stripped ELF file sits in a debug or profile library directory.
    ///
    /// Walks the Data files regular files and hardlinks whose File type is ELF
    /// without a static symbol table (`file(1)` "stripped"), under
    /// `lib/debug/` or `lib/profile/`, with or without a leading `usr/`, and
    /// that have content. Reported with the path as Location and an empty
    /// Extra. udebs are not skipped.
    StrippedLibrary,
    code = "LN1124",
    tag = "stripped-library",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for StrippedLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_file())
            .filter(|entry| in_debug_or_profile(entry.path().as_os_str().as_bytes()))
            .filter(|entry| {
                matches!(
                    entry.file_type(),
                    Some(FileType::Elf {
                        has_static_symbol_table: false,
                        ..
                    })
                )
            })
            .filter(|entry| files.read(entry).is_ok_and(|bytes| !bytes.is_empty()))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// lintian's `^ (?:usr/)? lib/ (?: debug | profile ) /`.
fn in_debug_or_profile(path: &[u8]) -> bool {
    let rest = path.strip_prefix(b"usr/").unwrap_or(path);
    rest.starts_with(b"lib/debug/") || rest.starts_with(b"lib/profile/")
}

#[cfg(test)]
mod tests {
    use crate::lint::file_type::FileType;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    /// An ELF file whose File type says whether it keeps its symbol table.
    fn elf(has_static_symbol_table: bool) -> TestFile {
        TestFile::new("\x7fELF stub").file_type(FileType::Elf {
            has_static_symbol_table,
            executable: false,
        })
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Not stripped: the symbol table is kept.
            BinaryPackageCase::from(vec![("usr/lib/debug/usr/share/foo/basic", elf(true))]),
            // Stripped, but empty.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/share/foo/empty",
                TestFile::new("").file_type(FileType::Elf {
                    has_static_symbol_table: false,
                    executable: false,
                }),
            )]),
            // Stripped, outside a debug or profile directory.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", elf(false))]),
            BinaryPackageCase::from(vec![("usr/lib/debugfoo/libfoo.so", elf(false))]),
            // Not an ELF file.
            BinaryPackageCase::from(vec![("usr/lib/debug/readme", TestFile::new("text"))]),
            // A symlink is not a file.
            BinaryPackageCase::from(vec![("usr/lib/debug/link", TestFile::symlink("x"))]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/lib/debug/usr/share/foo/basic", elf(false))]),
            BinaryPackageCase::from(vec![("lib/profile/libfoo.so", elf(false))]),
            BinaryPackageCase::from(vec![("lib/debug/libfoo.so.debug", elf(false))]),
            // A hardlink is a file.
            BinaryPackageCase::from(vec![("usr/lib/debug/hard", elf(false).hardlink())]),
            // A udeb is not skipped.
            BinaryPackageCase::from(vec![("usr/lib/debug/usr/share/foo/basic", elf(false))]).udeb(),
        ];
        BinaryPackageTester::new(StrippedLibrary, pass, fail).test_and_snapshot();
    }
}
