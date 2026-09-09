use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::file_type::FileType;
use crate::lint::rule::BinaryPackageRule;

pub struct UnstrippedBinaryOrObject;

declare_rule! {
    /// The Binary package installs an ELF with a static symbol table.
    ///
    /// Walks Data files (regular files and hardlinks). ELF File type must have
    /// a static symbol table. Skips paths ending in `.o` or `.ko`, packages
    /// whose name ends in `-dbg`, `lib/debug/` and `usr/lib/debug/`, Guile
    /// `.go`, `.gox`, and ELF executables whose GNU `strings --all`-style
    /// lines include `Caml1999X0` plus two digits. Extra is empty; the Data
    /// files path is the Location.
    UnstrippedBinaryOrObject,
    code = "LN0005",
    tag = "unstripped-binary-or-object",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnstrippedBinaryOrObject {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in unstripped_paths(files, &package) {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn unstripped_paths(files: &FileTree, package: &str) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter(|entry| is_unstripped(files, entry, package))
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

fn is_unstripped(files: &FileTree, entry: &Entry, package: &str) -> bool {
    let Some(FileType::Elf {
        has_static_symbol_table: true,
        executable,
    }) = entry.file_type()
    else {
        return false;
    };
    if excluded_path(entry.path()) || package.ends_with("-dbg") {
        return false;
    }
    if executable
        && files
            .read(entry)
            .is_ok_and(|bytes| contains_caml_bytecode(&bytes))
    {
        return false;
    }
    true
}

fn excluded_path(path: &Path) -> bool {
    let Some(name) = path.to_str() else {
        return false;
    };
    name.ends_with(".o")
        || name.ends_with(".ko")
        || name.starts_with("lib/debug/")
        || name.starts_with("usr/lib/debug/")
        || name.ends_with(".gox")
        || GUILE_PATH.is_match(name)
}

fn contains_caml_bytecode(bytes: &[u8]) -> bool {
    gnu_strings_all(bytes).any(is_caml_magic)
}

fn is_caml_magic(bytes: &[u8]) -> bool {
    matches!(
        bytes,
        [b'C', b'a', b'm', b'l', b'1', b'9', b'9', b'9', b'X', b'0', d1, d2]
            if d1.is_ascii_digit() && d2.is_ascii_digit()
    )
}

fn gnu_strings_all(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    struct Iter<'a> {
        rest: &'a [u8],
    }
    impl<'a> Iterator for Iter<'a> {
        type Item = &'a [u8];
        fn next(&mut self) -> Option<Self::Item> {
            loop {
                let start = self.rest.iter().position(|&byte| is_printable(byte))?;
                let rest = &self.rest[start..];
                let len = rest
                    .iter()
                    .position(|&byte| !is_printable(byte))
                    .unwrap_or(rest.len());
                let (string, tail) = rest.split_at(len);
                self.rest = tail;
                if string.len() >= 4 {
                    return Some(string);
                }
            }
        }
    }
    Iter { rest: bytes }
}

fn is_printable(byte: u8) -> bool {
    (b' '..=b'~').contains(&byte)
}

static GUILE_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib(?:/[^/]+)+/guile/[^/]+/.+\.go$").expect("Guile path regex")
});

#[cfg(test)]
mod tests {
    use crate::lint::file_tree::EntryKind;
    use crate::lint::file_type::FileType;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn unstripped(executable: bool) -> TestFile {
        TestFile::new("elf").file_type(FileType::Elf {
            has_static_symbol_table: true,
            executable,
        })
    }

    fn stripped(executable: bool) -> TestFile {
        TestFile::new("elf").file_type(FileType::Elf {
            has_static_symbol_table: false,
            executable,
        })
    }

    fn caml_bytecode_file(executable: bool) -> TestFile {
        let mut content = String::from("pad\n");
        content.push_str("Caml");
        content.push_str("1999");
        content.push_str("X0");
        content.push_str("11");
        content.push_str("\npad");
        TestFile::new(content).file_type(FileType::Elf {
            has_static_symbol_table: true,
            executable,
        })
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("usr/bin/app", stripped(true))]),
            BinaryPackageCase::from(vec![("usr/lib/foo.o", unstripped(false))]),
            BinaryPackageCase::from(vec![("usr/lib/modules/foo.ko", unstripped(false))]),
            BinaryPackageCase::from(vec![("usr/bin/app", unstripped(true))]).package("example-dbg"),
            BinaryPackageCase::from(vec![("usr/lib/debug/app", unstripped(true))]),
            BinaryPackageCase::from(vec![("lib/debug/app", unstripped(true))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/guile/3.0/site-ccache/foo.go",
                unstripped(false),
            )]),
            BinaryPackageCase::from(vec![("usr/lib/ocaml/foo.gox", unstripped(false))]),
            BinaryPackageCase::from(vec![("usr/bin/ocamlrun", caml_bytecode_file(true))]),
            BinaryPackageCase::from(vec![(
                "usr/bin/app.exe",
                TestFile::new("pe").file_type(FileType::Pe32),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/bin/link",
                TestFile::new("elf")
                    .kind(EntryKind::Symlink)
                    .file_type(FileType::Elf {
                        has_static_symbol_table: true,
                        executable: true,
                    }),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/bin/app", unstripped(true))]),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so", unstripped(false))]),
            BinaryPackageCase::from(vec![(
                "shared/payload",
                unstripped(true).kind(EntryKind::Hardlink),
            )]),
            BinaryPackageCase::from(vec![("usr/bin/app", unstripped(true))]).package("foo-dbgsym"),
            BinaryPackageCase::from(vec![("usr/lib/guile/3.0/foo.go", unstripped(false))]),
            BinaryPackageCase::from(vec![("usr/lib/libcaml.so", caml_bytecode_file(false))]),
        ];
        BinaryPackageTester::new(UnstrippedBinaryOrObject, pass, fail).test_and_snapshot();
    }
}
