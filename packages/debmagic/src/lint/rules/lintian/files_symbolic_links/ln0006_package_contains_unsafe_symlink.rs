use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsUnsafeSymlink;

declare_rule! {
    /// A symlink in the Binary package would escape the package root if followed.
    ///
    /// Walks Data files symlinks. The target is resolved lexically against the
    /// link's directory (absolute targets against the package root), dropping
    /// `.` and empty components; a `..` with nothing left to pop is unsafe.
    /// Targets that land on the package root itself are fine. Extra is empty;
    /// the Data files path is the Location.
    PackageContainsUnsafeSymlink,
    code = "LN0006",
    tag = "package-contains-unsafe-symlink",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsUnsafeSymlink {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let unsafe_paths: Vec<_> = files
            .query()
            .kinds([EntryKind::Symlink])
            .into_iter()
            .filter(|entry| is_unsafe(entry))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in unsafe_paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn is_unsafe(entry: &Entry) -> bool {
    let Some(target) = entry.link_target() else {
        return false;
    };
    escapes_package_root(entry.path().parent().unwrap_or(Path::new("")), target)
}

/// Lintian's `normalize_link_target`, reduced to "does it run out of bounds".
fn escapes_package_root(link_dir: &Path, target: &Path) -> bool {
    let target = target.as_os_str().as_bytes();
    let mut resolved: Vec<&[u8]> = Vec::new();
    if !target.starts_with(b"/") {
        resolved.extend(
            link_dir
                .components()
                .map(|component| component.as_os_str().as_bytes()),
        );
    }
    for component in target.split(|&byte| byte == b'/') {
        match component {
            b"" | b"." => {}
            b".." => {
                if resolved.pop().is_none() {
                    return true;
                }
            }
            other => resolved.push(other),
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn link(path: &str, target: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::symlink(target))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![link(
                "usr/lib/filenames/symlink1ok",
                "../../share/symlink",
            )]),
            BinaryPackageCase::from(vec![link(
                "usr/lib/filenames/symlink1wrong",
                "../../../etc/symlink",
            )]),
            BinaryPackageCase::from(vec![link("usr/lib/filenames/symlink2ok", "/etc/symlink")]),
            BinaryPackageCase::from(vec![link("usr/lib/filenames/symlink3ok", "test")]),
            BinaryPackageCase::from(vec![link(
                "usr/lib/filenames/symlink5wrong",
                "../menu/../somethingelse",
            )]),
            BinaryPackageCase::from(vec![link("usr/lib/filenames/symlink6wrong", "./file4")]),
            BinaryPackageCase::from(vec![link(
                "usr/lib/filenames/symlink8wrong",
                "../menu//something",
            )]),
            BinaryPackageCase::from(vec![link(
                "usr/lib/filenames/symlink9wrong",
                "../menu/something/",
            )]),
            // Resolving onto the package root itself is allowed.
            BinaryPackageCase::from(vec![link("usr/lib/filenames/to-usr-lib", "..")]),
            BinaryPackageCase::from(vec![link("usr/lib/filenames/to-self-dir", ".")]),
            BinaryPackageCase::from(vec![link("usr/lib/filenames/to-root", "/")]),
            BinaryPackageCase::from(vec![link("usr/lib/filenames/to-root-relative", "../../..")]),
            // Package-root-level link: one `..` would already be out of bounds, none is fine.
            BinaryPackageCase::from(vec![link("top", "usr/bin")]),
            BinaryPackageCase::from(vec![link("top", "/usr/bin/../lib")]),
            // Not symlinks (a regular file's content is not a target).
            BinaryPackageCase::from(vec![("usr/lib/file", "../../../../regular")]),
            BinaryPackageCase::from(vec![("usr/lib/dir", TestFile::directory())]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![link(
                "usr/lib/filenames/symlink2wrong",
                "../../../../etc/symlink",
            )]),
            BinaryPackageCase::from(vec![link(
                "usr/share/lintian/bad-links/absolute",
                "/../non-existent/path/lintian-should-not-open",
            )]),
            BinaryPackageCase::from(vec![link(
                "usr/share/lintian/bad-links/relative-escape",
                "../../../../../non-existent/path/lintian-should-not-open",
            )]),
            BinaryPackageCase::from(vec![link("top", "..")]),
            BinaryPackageCase::from(vec![link("usr/lib/link", "./../../../x")]),
            BinaryPackageCase::from(vec![link("usr/lib/link", "../../..//..")]),
            BinaryPackageCase::from(vec![
                link("usr/lib/ok", "../share"),
                link("usr/lib/bad", "../../../x"),
            ])
            .package("two-links"),
        ];
        BinaryPackageTester::new(PackageContainsUnsafeSymlink, pass, fail).test_and_snapshot();
    }
}
