use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Entry;
use crate::lint::rule::BinaryPackageRule;

pub struct BadOwnerForDocFile;

declare_rule! {
    /// A Data file under `usr/share/doc/` is not owned by `root/root` (or `0/0`).
    ///
    /// Walks every Data file (regular files, hardlinks, symlinks, directories
    /// alike, as lintian's `visit_installed_files` does) whose path starts with
    /// `usr/share/doc/` followed by a non-whitespace byte. Extra is the identity
    /// the Entry has, then `!= root/root (or 0/0)`; the Location is the Entry.
    /// An identity is `user/group` as the tar header gives it, with a number
    /// for an owner without a name (uid or gid `0` is `root`). An Entry without
    /// an owner (a host tree, not a Binary package) is skipped: there is no
    /// identity to judge. Deb and udeb are both checked.
    BadOwnerForDocFile,
    code = "LN1793",
    tag = "bad-owner-for-doc-file",
    default_selected = true,
    experimental = false,
}

const DOC_ROOT: &[u8] = b"usr/share/doc/";

impl BinaryPackageRule for BadOwnerForDocFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let reports: Vec<(PathBuf, String)> = files
            .entries()
            .iter()
            .filter(|entry| is_doc_path(entry))
            .filter_map(|entry| {
                let identity = entry.owner()?.identity();
                (identity != "root/root" && identity != "0/0").then(|| {
                    (
                        entry.path().to_path_buf(),
                        format!("{identity} != root/root (or 0/0)"),
                    )
                })
            })
            .collect();
        for (path, extra) in reports {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Lintian's `^usr/share/doc/\S` on the raw path bytes.
fn is_doc_path(entry: &Entry) -> bool {
    use std::os::unix::ffi::OsStrExt;

    entry
        .path()
        .as_os_str()
        .as_bytes()
        .strip_prefix(DOC_ROOT)
        .and_then(|rest| rest.first())
        // Perl's `\s`: space, tab, newline, carriage return, form feed, vertical tab.
        .is_some_and(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
}

#[cfg(test)]
mod tests {
    use crate::lint::file_tree::Owner;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data(
                "usr/share/doc/pkg/README",
                TestFile::new("x").owned_by("root", "root"),
            ),
            // `0/0` by number, without names.
            data(
                "usr/share/doc/pkg/README",
                TestFile::new("x").owner(Owner::new(0, 0, None, None)),
            ),
            // No owner (host tree): nothing to judge.
            data("usr/share/doc/pkg/README", TestFile::new("x")),
            // Outside `usr/share/doc/`, or not followed by a non-space byte.
            data("usr/bin/tool", TestFile::new("x").owned_by("user", "user")),
            data(
                "usr/share/docs/README",
                TestFile::new("x").owned_by("user", "user"),
            ),
            data(
                "usr/share/doc/",
                TestFile::directory().owned_by("user", "user"),
            ),
            data(
                "etc/pkg/README",
                TestFile::new("x").owned_by("user", "user"),
            ),
            // Root-owned directories, symlinks and hardlinks are fine.
            data(
                "usr/share/doc/pkg",
                TestFile::directory().owned_by("root", "root"),
            ),
            data(
                "usr/share/doc/pkg/link",
                TestFile::symlink("README").owned_by("root", "root"),
            ),
        ];
        let fail = vec![
            data(
                "usr/share/doc/pkg/README",
                TestFile::new("x").owned_by("user", "user"),
            ),
            data(
                "usr/share/doc/pkg/README",
                TestFile::new("x").owned_by("root", "games"),
            ),
            // An owner without a name prints its number.
            data(
                "usr/share/doc/pkg/README",
                TestFile::new("x").owner(Owner::new(1000, 0, None, None)),
            ),
            // Directories, symlinks and hardlinks are checked too.
            data(
                "usr/share/doc/pkg",
                TestFile::directory().owned_by("user", "user"),
            ),
            data(
                "usr/share/doc/pkg/link",
                TestFile::symlink("README").owned_by("user", "user"),
            ),
            data(
                "usr/share/doc/pkg/hard",
                TestFile::new("x").hardlink().owned_by("user", "user"),
            ),
            // udebs are checked too.
            data(
                "usr/share/doc/pkg/README",
                TestFile::new("x").owned_by("user", "user"),
            )
            .udeb(),
        ];
        BinaryPackageTester::new(BadOwnerForDocFile, pass, fail).test_and_snapshot();
    }
}
