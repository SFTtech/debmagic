use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::group_context::GroupContext;
use crate::lint::rule::GroupRule;

pub struct PackageContainsBrokenSymlinkWildcard;

declare_rule! {
    /// A symlink whose target contains `*` (a failed `ln -s target/*.so`
    /// expansion) leads to nothing in its package or in any installable of
    /// the Group it depends on. The extra is the target with its leading
    /// slashes dropped; the symlink is the Location.
    ///
    /// The target is normalized against the symlink's directory (absolute
    /// targets against the package root); one that leaves the package, or
    /// leads to the package root, is not looked at. The destination is looked
    /// up in the package's own files and in those of the installables its
    /// Pre-Depends and Depends name (the Group's direct dependencies), as a
    /// file or a directory. Other broken symlinks are not looked for.
    ///
    /// A Group Rule: it needs the dependencies' files, so it runs once per
    /// Group that has a `.dsc`, and the Diagnostic attaches to the
    /// installable that has the symlink, as in lintian.
    PackageContainsBrokenSymlinkWildcard,
    code = "LN1504",
    tag = "package-contains-broken-symlink-wildcard",
    default_selected = true,
    experimental = false,
}

impl GroupRule for PackageContainsBrokenSymlinkWildcard {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        for installable in ctx.installables() {
            let Some(files) = ctx.data_files(&installable) else {
                continue;
            };
            let wildcards: Vec<(PathBuf, PathBuf, String)> = files
                .query()
                .kinds([EntryKind::Symlink])
                .into_iter()
                .filter_map(|entry| {
                    let link = entry.link_target()?;
                    if !link.as_os_str().as_bytes().contains(&b'*') {
                        return None;
                    }
                    let normalized = normalize_link_target(
                        entry.path().parent().unwrap_or(Path::new("")),
                        link,
                    )?;
                    if normalized.is_empty() {
                        return None;
                    }
                    Some((
                        entry.path().to_path_buf(),
                        PathBuf::from(OsStr::from_bytes(&normalized)),
                        link.to_string_lossy().trim_start_matches('/').to_string(),
                    ))
                })
                .collect();
            if wildcards.is_empty() {
                continue;
            }
            let mut prerequisites = Vec::new();
            for dependency in ctx.direct_dependencies(&installable) {
                if let Some(tree) = ctx.data_files(&dependency) {
                    prerequisites.push(tree);
                }
            }
            for (path, destination, target) in wildcards {
                if files.get(&destination).is_some()
                    || prerequisites
                        .iter()
                        .any(|tree| tree.get(&destination).is_some())
                {
                    continue;
                }
                ctx.installable_diagnostic(
                    &installable,
                    Diagnostic::warn(target).with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
                );
            }
        }
    }
}

/// lintian's `normalize_link_target`: the target from the link's directory
/// (from the root when absolute) with `.` and empty components dropped and
/// `..` resolved. `None` when a `..` leaves the package; empty when it ends
/// at the root.
fn normalize_link_target(directory: &Path, target: &Path) -> Option<Vec<u8>> {
    let target = target.as_os_str().as_bytes();
    let mut resolved: Vec<&[u8]> = Vec::new();
    if !target.starts_with(b"/") {
        resolved.extend(
            directory
                .components()
                .map(|component| component.as_os_str().as_bytes()),
        );
    }
    for component in target.split(|&byte| byte == b'/') {
        match component {
            b"" | b"." => {}
            b".." => {
                resolved.pop()?;
            }
            other => resolved.push(other),
        }
    }
    Some(resolved.join(&b'/'))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester, TestFile};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";

    fn link(path: &str, target: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::symlink(target))
    }

    #[test]
    fn normalizes_like_lintian() {
        let normalized = |dir: &str, target: &str| {
            normalize_link_target(Path::new(dir), Path::new(target))
                .map(|bytes| String::from_utf8(bytes).unwrap())
        };
        assert_eq!(
            normalized("etc", "/etc/some-*.conf").as_deref(),
            Some("etc/some-*.conf")
        );
        assert_eq!(
            normalized("usr/lib/x", "../y/./*.so").as_deref(),
            Some("usr/lib/y/*.so")
        );
        assert_eq!(normalized("etc", "../*").as_deref(), Some("*"));
        assert_eq!(normalized("etc", "..").as_deref(), Some(""));
        assert_eq!(normalized("etc", "../../*"), None);
    }

    #[test]
    fn group() {
        let pass = vec![
            // No wildcard.
            GroupCase::new(DSC)
                .installable("a", "")
                .files([link("etc/x.conf", "/etc/missing")]),
            // The destination exists, as a file or a directory, in the package.
            GroupCase::new(DSC).installable("a", "").files(vec![
                link("etc/x.conf", "/etc/some-*.conf"),
                ("etc/some-*.conf".to_string(), TestFile::new("")),
            ]),
            GroupCase::new(DSC).installable("a", "").files(vec![
                link("etc/x.conf", "some-*"),
                ("etc/some-*".to_string(), TestFile::directory()),
            ]),
            // The destination is in a direct dependency.
            GroupCase::new(DSC)
                .installable("a", "Depends: b\n")
                .files([link("etc/x.conf", "/etc/some-*.conf")])
                .installable("b", "")
                .files([("etc/some-*.conf", TestFile::new(""))]),
            // A target that leaves the package or leads to the root is not looked at.
            GroupCase::new(DSC)
                .installable("a", "")
                .files([link("etc/x.conf", "../../*")]),
            GroupCase::new(DSC)
                .installable("a", "")
                .files([link("etc/x.conf", "..")]),
        ];
        let fail = vec![
            GroupCase::new(DSC)
                .installable("a", "")
                .files([link("etc/x.conf", "/etc/some-*.conf")]),
            // Relative target, in the second of two packages.
            GroupCase::new(DSC)
                .installable("a", "")
                .installable("b", "")
                .files([link("usr/lib/y/libz.so", "../x/*.so")]),
            // Not a dependency (weak, or the other way round), or the udeb.
            GroupCase::new(DSC)
                .installable("a", "Recommends: b\n")
                .files([link("etc/x.conf", "/etc/some-*.conf")])
                .installable("b", "")
                .files([("etc/some-*.conf", TestFile::new(""))]),
            GroupCase::new(DSC)
                .installable("a", "")
                .files([link("etc/x.conf", "///etc/some-*.conf")])
                .udeb("c-udeb", "")
                .files([link("etc/y.conf", "other-*")]),
        ];
        GroupTester::new(PackageContainsBrokenSymlinkWildcard, pass, fail).test_and_snapshot();
    }
}
