use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::ln0721_multiarch_foreign_pkgconfig::HOST_MULTIARCH;

pub struct MultiarchForeignCmakeFile;

declare_rule! {
    /// The Binary package is `Multi-Arch: foreign` but ships a cmake file in
    /// its architecture's public cmake directory.
    ///
    /// Only when Multi-Arch is exactly `foreign` and Architecture has a
    /// DEB_HOST_MULTIARCH triplet (`all` has none). Walks the Data files that
    /// are not directories (symlinks and hardlinks included) and reports each
    /// `usr/lib/<triplet>/cmake/<name>.cmake` at its path, with an empty extra.
    /// `<name>` may hold slashes. Udebs are checked too.
    MultiarchForeignCmakeFile,
    code = "LN0765",
    tag = "multiarch-foreign-cmake-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MultiarchForeignCmakeFile {
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
                    .is_some_and(|name| is_public_cmake(name, &triplet))
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

/// `usr/lib/<triplet>/cmake/<name>.cmake`, with at least one character before `.cmake`.
fn is_public_cmake(name: &str, triplet: &str) -> bool {
    let Some(tail) = name
        .strip_prefix("usr/lib/")
        .and_then(|rest| rest.strip_prefix(triplet))
        .and_then(|rest| rest.strip_prefix('/'))
    else {
        return false;
    };
    let Some(file) = tail.strip_prefix("cmake/") else {
        return false;
    };
    file.len() > ".cmake".len() && file.ends_with(".cmake")
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn cmake() -> TestFile {
        TestFile::new("set(FOO 1)\n")
    }

    fn foreign_amd64(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
            .field("Architecture", "amd64")
            .field("Multi-Arch", "foreign")
    }

    #[test]
    fn public_cmake_paths() {
        let check = |name: &str| is_public_cmake(name, "x86_64-linux-gnu");
        assert!(check("usr/lib/x86_64-linux-gnu/cmake/foo.cmake"));
        assert!(check("usr/lib/x86_64-linux-gnu/cmake/foo/bar.cmake"));
        assert!(check("usr/lib/x86_64-linux-gnu/cmake/a.cmake"));
        assert!(!check("usr/lib/x86_64-linux-gnu/cmake/.cmake"));
        assert!(!check("usr/lib/x86_64-linux-gnu/cmake/foo.cmake.in"));
        assert!(!check("usr/lib/x86_64-linux-gnu/cmake/"));
        assert!(!check("usr/lib/x86_64-linux-gnu/foo.cmake"));
        assert!(!check("usr/lib/i386-linux-gnu/cmake/foo.cmake"));
        assert!(!check("usr/lib/cmake/foo.cmake"));
        assert!(!check("usr/lib/x86_64-linux-gnu2/cmake/foo.cmake"));
    }

    #[test]
    fn test() {
        let pass = vec![
            // Only Multi-Arch: foreign is judged.
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/cmake/foo.cmake", cmake())])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/cmake/foo.cmake", cmake())])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            // Architecture all has no DEB_HOST_MULTIARCH triplet.
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/cmake/foo.cmake", cmake())])
                .field("Architecture", "all")
                .field("Multi-Arch", "foreign"),
            foreign_amd64(vec![("usr/lib/cmake/foo.cmake", cmake())]),
            foreign_amd64(vec![("usr/lib/i386-linux-gnu/cmake/foo.cmake", cmake())]),
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/foo.cmake", cmake())]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/cmake/foo.cmake.in",
                cmake(),
            )]),
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/cmake/.cmake", cmake())]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/cmake",
                TestFile::directory(),
            )]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/cmake/foo.cmake",
                TestFile::directory(),
            )]),
            foreign_amd64(vec![("usr/share/cmake/foo.cmake", cmake())]),
            // A `.cmake` in a directory whose architecture has no triplet is not judged.
            BinaryPackageCase::from(vec![("usr/lib/foo/cmake/foo.cmake", cmake())])
                .field("Architecture", "lintian-fake-arch")
                .field("Multi-Arch", "foreign"),
        ];
        let fail = vec![
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/cmake/foo.cmake", cmake())]),
            foreign_amd64(vec![
                ("usr/lib/x86_64-linux-gnu/cmake/a.cmake", cmake()),
                ("usr/lib/x86_64-linux-gnu/cmake/b.cmake", cmake()),
            ]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/cmake/foo/bar.cmake",
                cmake(),
            )]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/cmake/link.cmake",
                TestFile::symlink("foo.cmake"),
            )]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/cmake/hard.cmake",
                cmake().hardlink(),
            )]),
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/cmake/foo.cmake", cmake())])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "foreign")
                .udeb(),
        ];
        BinaryPackageTester::new(MultiarchForeignCmakeFile, pass, fail).test_and_snapshot();
    }
}
