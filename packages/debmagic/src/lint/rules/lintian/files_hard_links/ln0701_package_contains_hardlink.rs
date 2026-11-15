use std::collections::BTreeMap;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsHardlink;

declare_rule! {
    /// A hardlink lives in `/etc`, points into `/etc`, or crosses directories.
    ///
    /// Walks Data files hardlinks after lintian's calibration: each original
    /// and the hardlinks to it form a group, the name that sorts first (by
    /// bytes) is the regular file, and every other name is a hardlink to it.
    /// A hardlink is reported when its name or its target starts with `etc/`,
    /// or when it is not in the target's directory. Extra is
    /// `pointing to: <target>`; the hardlink's path is the Location. Udebs
    /// are checked too.
    PackageContainsHardlink,
    code = "LN0701",
    tag = "package-contains-hardlink",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsHardlink {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = calibrated_hardlinks(files)
            .into_iter()
            .filter(|(name, target)| is_reported(name, target))
            .collect();
        for (name, target) in hits {
            ctx.diagnostic(
                Diagnostic::warn(format!("pointing to: {}", target.display())).with_location(
                    Location {
                        path: name,
                        line: None,
                        column: None,
                    },
                ),
            );
        }
    }
}

/// Lintian's `Lintian::Index` hardlink calibration: `(hardlink, target)`
/// pairs where the target is the group's name that sorts first by bytes.
fn calibrated_hardlinks(files: &FileTree) -> Vec<(PathBuf, PathBuf)> {
    let mut groups: BTreeMap<&Path, Vec<&Path>> = BTreeMap::new();
    for entry in files.entries() {
        if let Some(target) = entry.hardlink_target() {
            groups.entry(target).or_default().push(entry.path());
        }
    }
    let mut pairs = Vec::new();
    for (original, mut group) in groups {
        group.push(original);
        group.sort_by_key(|name| name.as_os_str().as_bytes());
        group.dedup();
        let (preferred, links) = group.split_first().expect("group has its original");
        pairs.extend(
            links
                .iter()
                .map(|link| (link.to_path_buf(), preferred.to_path_buf())),
        );
    }
    pairs
}

/// The Check's test: in or into `etc/`, or outside the target's directory.
fn is_reported(name: &Path, target: &Path) -> bool {
    let name = name.as_os_str().as_bytes();
    let target = target.as_os_str().as_bytes();
    if name.starts_with(b"etc/") || target.starts_with(b"etc/") {
        return true;
    }
    let target_dir = match target.iter().rposition(|&byte| byte == b'/') {
        Some(slash) => &target[..=slash],
        None => &[][..],
    };
    match name.strip_prefix(target_dir) {
        Some(rest) => rest.contains(&b'/'),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::file_tree::Entry;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
    }

    fn file() -> TestFile {
        TestFile::new("Hallo world\n")
    }

    fn link(target: &str) -> TestFile {
        file().hardlink_to(target)
    }

    #[test]
    fn reported_names() {
        let reported = |name: &str, target: &str| is_reported(Path::new(name), Path::new(target));
        assert!(!reported("usr/bin/static-hello", "usr/bin/hello.static"));
        assert!(!reported("usr/foo", "usr/bar"));
        assert!(!reported("top-b", "top-a"));
        assert!(reported("usr/share/baz", "usr/bar2"));
        assert!(reported("usr/bar2/x", "usr/bar2"));
        assert!(reported("etc/improper-link", "etc/improper"));
        assert!(reported("usr/share/b", "etc/a"));
        assert!(reported("etc/b", "usr/share/a"));
        assert!(reported("usr/sub/b", "usr/a"));
        assert!(reported("top", "usr/a"));
    }

    #[test]
    fn calibration_points_to_the_first_name() {
        let files = FileTree::from_entries(
            None,
            vec![
                Entry::regular_file("a/orig", None, None),
                Entry::hardlink("b/link", None, None).with_hardlink_target("a/orig"),
                Entry::hardlink("a-z/link", None, None).with_hardlink_target("a/orig"),
            ],
        );
        // Bytes, not path components: `a-z/link` sorts before `a/orig`.
        assert_eq!(
            calibrated_hardlinks(&files),
            vec![
                (PathBuf::from("a/orig"), PathBuf::from("a-z/link")),
                (PathBuf::from("b/link"), PathBuf::from("a-z/link")),
            ]
        );
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![]),
            tree(vec![
                ("usr/bin/hello.static", file()),
                ("usr/bin/static-hello", link("usr/bin/hello.static")),
            ]),
            // A hardlink without a known target is not judged.
            tree(vec![("usr/share/a/x", file().hardlink())]),
            // Regular files and symlinks are not hardlinks.
            tree(vec![
                ("etc/foo", file()),
                ("usr/share/foo", TestFile::symlink("../../etc/foo")),
            ]),
        ];
        let fail = vec![
            tree(vec![
                ("usr/bar2", file()),
                ("usr/share/baz", link("usr/bar2")),
            ]),
            tree(vec![
                ("etc/improper", file()),
                ("etc/improper-link", link("etc/improper")),
            ]),
            tree(vec![
                ("usr/share/p/3rd/orig", file()),
                ("usr/share/p/4th/link1", link("usr/share/p/3rd/orig")),
                ("usr/share/p/1st/link2", link("usr/share/p/3rd/orig")),
                ("usr/share/p/2nd/link3", link("usr/share/p/3rd/orig")),
            ]),
            tree(vec![
                ("usr/lib/a", file()),
                ("usr/lib/sub/b", link("usr/lib/a")),
            ])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsHardlink, pass, fail).test_and_snapshot();
    }
}
