use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::ln0721_multiarch_foreign_pkgconfig::HOST_MULTIARCH;

pub struct MultiarchForeignStaticLibrary;

declare_rule! {
    /// The Binary package is `Multi-Arch: foreign` but ships a static library
    /// in its architecture's public library directory.
    ///
    /// Only when Multi-Arch is exactly `foreign` and Architecture has a
    /// DEB_HOST_MULTIARCH triplet (`all` has none). Walks the Data files that
    /// are not directories (symlinks and hardlinks included) and reports each
    /// `usr/lib/<triplet>/lib<name>.a` at its path, with an empty extra. The
    /// file name is `lib` plus at least one character plus `.a`, with no
    /// further directory. Udebs are checked too.
    MultiarchForeignStaticLibrary,
    code = "LN0767",
    tag = "multiarch-foreign-static-library",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MultiarchForeignStaticLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let fields = ctx.installation_control().fields();
        if fields.get("Multi-Arch") != Some("foreign") {
            return;
        }
        let Some(triplet) = fields
            .get("Architecture")
            .and_then(|architecture| HOST_MULTIARCH.get(architecture))
            .cloned()
        else {
            return;
        };
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|name| is_public_static_library(name, &triplet))
            })
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

/// `usr/lib/<triplet>/lib<name>.a`, `<name>` one or more characters without a slash.
fn is_public_static_library(name: &str, triplet: &str) -> bool {
    let Some(tail) = name
        .strip_prefix("usr/lib/")
        .and_then(|rest| rest.strip_prefix(triplet))
        .and_then(|rest| rest.strip_prefix('/'))
    else {
        return false;
    };
    let Some(stem) = tail
        .strip_prefix("lib")
        .and_then(|rest| rest.strip_suffix(".a"))
    else {
        return false;
    };
    !stem.is_empty() && !stem.contains('/')
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

    fn foreign_amd64(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
            .field("Architecture", "amd64")
            .field("Multi-Arch", "foreign")
    }

    #[test]
    fn public_static_library_paths() {
        let check = |name: &str| is_public_static_library(name, "x86_64-linux-gnu");
        assert!(check("usr/lib/x86_64-linux-gnu/libfoo.a"));
        assert!(check("usr/lib/x86_64-linux-gnu/liba.a"));
        assert!(!check("usr/lib/x86_64-linux-gnu/lib.a"));
        assert!(!check("usr/lib/x86_64-linux-gnu/libfoo.a.in"));
        assert!(!check("usr/lib/x86_64-linux-gnu/sub/libfoo.a"));
        assert!(!check("usr/lib/x86_64-linux-gnu/foo.a"));
        assert!(!check("usr/lib/x86_64-linux-gnu/libfoo.so"));
        assert!(!check("usr/lib/i386-linux-gnu/libfoo.a"));
        assert!(!check("usr/lib/libfoo.a"));
    }

    #[test]
    fn test() {
        let pass = vec![
            // Only Multi-Arch: foreign is judged.
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/libfoo.a", archive())])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/libfoo.a", archive())])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            // Architecture all has no DEB_HOST_MULTIARCH triplet.
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/libfoo.a", archive())])
                .field("Architecture", "all")
                .field("Multi-Arch", "foreign"),
            foreign_amd64(vec![("usr/lib/libfoo.a", archive())]),
            foreign_amd64(vec![("usr/lib/i386-linux-gnu/libfoo.a", archive())]),
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/lib.a", archive())]),
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/sub/libfoo.a", archive())]),
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/libfoo.so", archive())]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/libfoo.a",
                TestFile::directory(),
            )]),
            foreign_amd64(vec![("usr/share/libfoo.a", archive())]),
            // A `.a` in a directory whose architecture has no triplet is not judged.
            BinaryPackageCase::from(vec![("usr/lib/foo/libfoo.a", archive())])
                .field("Architecture", "lintian-fake-arch")
                .field("Multi-Arch", "foreign"),
        ];
        let fail = vec![
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/libfoo.a", archive())]),
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/liba.a", archive())]),
            foreign_amd64(vec![
                ("usr/lib/x86_64-linux-gnu/libfoo.a", archive()),
                ("usr/lib/x86_64-linux-gnu/libbar.a", archive()),
            ]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/libfoo.a",
                TestFile::symlink("libfoo.a.1"),
            )]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/libhard.a",
                archive().hardlink(),
            )]),
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/libfoo.a", archive())])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "foreign")
                .udeb(),
        ];
        BinaryPackageTester::new(MultiarchForeignStaticLibrary, pass, fail).test_and_snapshot();
    }
}
