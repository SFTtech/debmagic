use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct DirOrFileInBuildTree;

declare_rule! {
    /// The Binary package ships a file or directory under a common build path.
    ///
    /// Walks Data files entries (directories with a trailing slash) and flags
    /// a path that starts with `var/cache/pbuilder/build/`, `var/lib/sbuild/`,
    /// `var/lib/buildd/`, `build/` or `tmp/buildd/`. Packages whose source is
    /// `sbuild` or `pbuilder` are skipped. Udebs are checked too. Extra is
    /// empty; the Data files path (with a trailing slash for directories) is
    /// the Location.
    DirOrFileInBuildTree,
    code = "LN0728",
    tag = "dir-or-file-in-build-tree",
    default_selected = true,
    experimental = false,
}

/// Lintian's `files/build-path-regex` data, each pattern anchored at the start.
const BUILD_PATHS: &[&[u8]] = &[
    b"var/cache/pbuilder/build/",
    b"var/lib/sbuild/",
    b"var/lib/buildd/",
    b"build/",
    b"tmp/buildd/",
];

impl BinaryPackageRule for DirOrFileInBuildTree {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let source = ctx.source_name();
        if source == "sbuild" || source == "pbuilder" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter_map(|entry| {
                let mut name = entry.path().as_os_str().as_encoded_bytes().to_vec();
                if entry.kind() == EntryKind::Directory {
                    name.push(b'/');
                }
                BUILD_PATHS
                    .iter()
                    .any(|prefix| name.starts_with(prefix))
                    .then(|| PathBuf::from(OsString::from_vec(name)))
            })
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(entries: &[(&str, bool)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, dir)| {
                    let file = if *dir {
                        TestFile::directory()
                    } else {
                        TestFile::new("x")
                    };
                    (path.to_string(), file)
                })
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            // A file named like a build path directory is not inside it.
            tree(&[
                ("build", false),
                ("tmp/buildd", false),
                ("var/lib/sbuild", false),
            ]),
            // The prefixes are anchored at the start of the path.
            tree(&[
                ("other/build/file", false),
                ("usr/share/build/file", false),
                ("var/lib/buildd-other/file", false),
                ("var/cache/pbuilder/file", false),
                ("tmp/buildd-x/file", false),
                ("tmp/file", false),
                ("usr/build/", true),
            ]),
            // The source package is exempt.
            tree(&[("build/file", false)]).field("Source", "sbuild"),
            tree(&[("build/file", false)]).field("Source", "pbuilder (1.0)"),
        ];
        let fail = vec![
            tree(&[("build/dir/foo/bar", false)]),
            tree(&[("build/dir", true), ("build/dir/foo/bar", false)]),
            tree(&[("var/lib/sbuild/bar", false)]),
            tree(&[("var/lib/buildd/bar", false)]),
            tree(&[("var/cache/pbuilder/build/bar", false)]),
            tree(&[("tmp/buildd/dir/foo/bar", false)]),
            // Udebs are checked too.
            tree(&[("build/file", false)]).udeb(),
            // Other sources are checked.
            tree(&[("build/file", false)]).field("Source", "other (1.0)"),
        ];
        BinaryPackageTester::new(DirOrFileInBuildTree, pass, fail).test_and_snapshot();
    }
}
