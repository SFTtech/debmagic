use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::ln0721_multiarch_foreign_pkgconfig::HOST_MULTIARCH;

pub struct MultiarchForeignSharedLibrary;

declare_rule! {
    /// The Binary package is architecture-dependent, `Multi-Arch: foreign`,
    /// ships a shared library in a public library directory and ships no
    /// executable in a public binary directory.
    ///
    /// Only when Architecture is not `all` and Multi-Arch is exactly `foreign`.
    /// A public shared library is `usr/lib/libfoo.so` (`usr/` optional), or
    /// the same under a DEB_HOST_MULTIARCH triplet directory. A public
    /// executable is any entry whose parent is `bin`, `sbin`, `usr/bin`,
    /// `usr/sbin` or `usr/games`, directories included. Reported once per
    /// package, without a location. Udebs are checked too.
    MultiarchForeignSharedLibrary,
    code = "LN0766",
    tag = "multiarch-foreign-shared-library",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MultiarchForeignSharedLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let fields = ctx.installation_control().fields();
        if fields.get("Architecture") == Some("all") || fields.get("Multi-Arch") != Some("foreign")
        {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let entries = files.entries();
        let has_shared_library = entries
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .any(|entry| entry.path().to_str().is_some_and(is_public_shared_library));
        if !has_shared_library {
            return;
        }
        let has_public_executable = entries.iter().any(|entry| {
            entry
                .path()
                .parent()
                .and_then(|dir| dir.to_str())
                .is_some_and(|dir| PUBLIC_EXECUTABLE_DIRS.contains(&dir))
        });
        if !has_public_executable {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

const PUBLIC_EXECUTABLE_DIRS: [&str; 5] = ["bin", "sbin", "usr/bin", "usr/sbin", "usr/games"];

/// `usr/lib/libfoo.so` or `usr/lib/<triplet>/libfoo.so`, `usr/` optional. The
/// file name starts with `lib` and ends with `.so`; the triplet must be known.
fn is_public_shared_library(name: &str) -> bool {
    let Some(rest) = name
        .strip_prefix("usr/lib/")
        .or_else(|| name.strip_prefix("lib/"))
    else {
        return false;
    };
    let file = match rest.split_once('/') {
        None => rest,
        Some((triplet, file)) => {
            if file.contains('/') || !HOST_MULTIARCH.values().any(|known| known == triplet) {
                return false;
            }
            file
        }
    };
    file.len() >= "lib.so".len() && file.starts_with("lib") && file.ends_with(".so")
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file() -> TestFile {
        TestFile::new("ELF-ish\n")
    }

    fn foreign_amd64(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
            .field("Architecture", "amd64")
            .field("Multi-Arch", "foreign")
    }

    #[test]
    fn public_shared_library_paths() {
        assert!(is_public_shared_library("usr/lib/libfoo.so"));
        assert!(is_public_shared_library("lib/libfoo.so"));
        assert!(is_public_shared_library(
            "usr/lib/x86_64-linux-gnu/libfoo.so"
        ));
        assert!(is_public_shared_library("usr/lib/lib.so"));
        assert!(!is_public_shared_library("usr/lib/libfoo.so.1"));
        assert!(!is_public_shared_library("usr/lib/foo.so"));
        assert!(!is_public_shared_library(
            "usr/lib/x86_64-linux-gnu/sub/libfoo.so"
        ));
        assert!(!is_public_shared_library("usr/lib/perl/libfoo.so"));
        assert!(!is_public_shared_library("usr/lib//libfoo.so"));
        assert!(!is_public_shared_library("usr/share/libfoo.so"));
        assert!(!is_public_shared_library(
            "usr/lib/x86_64-linux-gnu/libfoo.a"
        ));
    }

    #[test]
    fn test() {
        let pass = vec![
            // Only Multi-Arch: foreign is judged.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so", file())])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so", file())])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            // Architecture all is exempt.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so", file())])
                .field("Architecture", "all")
                .field("Multi-Arch", "foreign"),
            // A public executable exempts the package.
            foreign_amd64(vec![
                ("usr/lib/x86_64-linux-gnu/libfoo.so", file()),
                ("usr/bin/foo", file()),
            ]),
            foreign_amd64(vec![
                ("usr/lib/libfoo.so", file()),
                ("usr/sbin/foo", file()),
            ]),
            foreign_amd64(vec![
                ("usr/lib/libfoo.so", file()),
                ("usr/games/foo", file()),
            ]),
            foreign_amd64(vec![("usr/lib/libfoo.so", file()), ("bin/foo", file())]),
            foreign_amd64(vec![
                ("usr/lib/libfoo.so", file()),
                ("usr/bin/sub", TestFile::directory()),
            ]),
            // Not a public shared library.
            foreign_amd64(vec![("usr/lib/libfoo.so.1", file())]),
            foreign_amd64(vec![("usr/lib/perl/libfoo.so", file())]),
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/sub/libfoo.so", file())]),
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/libfoo.a", file())]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/libfoo.so",
                TestFile::directory(),
            )]),
            foreign_amd64(vec![("usr/share/libfoo.so", file())]),
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/foo.so", file())]),
        ];
        let fail = vec![
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/libfoo.so", file())]),
            foreign_amd64(vec![("usr/lib/libfoo.so", file())]),
            foreign_amd64(vec![("lib/libfoo.so", file())]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/libfoo.so",
                TestFile::symlink("libfoo.so.1"),
            )]),
            // Only the listed directories count, not nested ones.
            foreign_amd64(vec![
                ("usr/lib/libfoo.so", file()),
                ("usr/bin/sub/foo", file()),
            ]),
            // A directory alone is not an executable.
            foreign_amd64(vec![
                ("usr/lib/x86_64-linux-gnu/libfoo.so", file()),
                ("usr/bin", TestFile::directory()),
            ]),
            // Udebs are checked too.
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/libfoo.so", file())])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "foreign")
                .udeb(),
        ];
        BinaryPackageTester::new(MultiarchForeignSharedLibrary, pass, fail).test_and_snapshot();
    }
}
