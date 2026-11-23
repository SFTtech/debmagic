use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Entry;
use crate::lint::rule::BinaryPackageRule;

pub struct WrongFileOwnerUidOrGid;

declare_rule! {
    /// A Data file's owner or group ID is outside the globally allocated IDs.
    ///
    /// Walks every Data file (regular files, hardlinks, symlinks, directories
    /// alike, as lintian's `visit_installed_files` does). The allowed IDs are
    /// `0` to `99`, `65534`, and `60000` to `64999`; anything else is reported.
    /// Extra is `<uid>/<gid>` in numbers; the Location is the Entry. Deb and
    /// udeb are both checked. An Entry without an owner (a host tree, not a
    /// Binary package) is skipped: there is no ID to judge.
    ///
    /// The Perl code allows `60000` to `64999`, while the Tag's explanation says
    /// `64000` to `64999`. The code is ported as it runs, so `60000` to `63999`
    /// is allowed here too.
    WrongFileOwnerUidOrGid,
    code = "LN1794",
    tag = "wrong-file-owner-uid-or-gid",
    default_selected = true,
    experimental = false,
}

const MAXIMUM_LOW_RESERVED: u64 = 99;
const MINIMUM_HIGH_RESERVED: u64 = 60_000;
const MAXIMUM_HIGH_RESERVED: u64 = 64_999;
const NOBODY: u64 = 65_534;

impl BinaryPackageRule for WrongFileOwnerUidOrGid {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let reports: Vec<(PathBuf, String)> = files
            .entries()
            .iter()
            .filter_map(|entry| {
                let (uid, gid) = ids(entry)?;
                (out_of_bounds(uid) || out_of_bounds(gid))
                    .then(|| (entry.path().to_path_buf(), format!("{uid}/{gid}")))
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

/// The owner's `(uid, gid)`, when the Entry has an owner.
fn ids(entry: &Entry) -> Option<(u64, u64)> {
    let owner = entry.owner()?;
    Some((owner.uid(), owner.gid()))
}

/// Lintian's `out_of_bounds`: not in the low reserved range, not `nobody`, and
/// not in the high reserved range.
fn out_of_bounds(id: u64) -> bool {
    !(id <= MAXIMUM_LOW_RESERVED
        || id == NOBODY
        || (MINIMUM_HIGH_RESERVED..=MAXIMUM_HIGH_RESERVED).contains(&id))
}

#[cfg(test)]
mod tests {
    use crate::lint::file_tree::Owner;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    fn owned(uid: u64, gid: u64) -> TestFile {
        TestFile::new("x").owner(Owner::new(uid, gid, None, None))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Edges of each allowed range.
            data("usr/bin/a", owned(0, 0)),
            data("usr/bin/a", owned(99, 99)),
            data("usr/bin/a", owned(65_534, 65_534)),
            data("usr/bin/a", owned(60_000, 60_000)),
            data("usr/bin/a", owned(64_999, 64_999)),
            // Named root and the usual user.
            data("usr/bin/a", TestFile::new("x").owned_by("root", "root")),
            // No owner (host tree): nothing to judge.
            data("usr/bin/a", TestFile::new("x")),
            // Directories, symlinks and hardlinks in range are fine.
            data(
                "usr/share/doc/pkg",
                TestFile::directory().owned_by("root", "root"),
            ),
            data(
                "usr/bin/link",
                TestFile::symlink("a").owned_by("root", "root"),
            ),
            data(
                "usr/bin/hard",
                TestFile::new("x").hardlink().owned_by("root", "root"),
            ),
            // Udebs too.
            data("usr/bin/a", owned(0, 0)).udeb(),
        ];
        let fail = vec![
            // Just outside each allowed range.
            data("usr/bin/a", owned(100, 0)),
            data("usr/bin/a", owned(0, 100)),
            data("usr/bin/a", owned(65_535, 0)),
            data("usr/bin/a", owned(59_999, 0)),
            data("usr/bin/a", owned(65_000, 0)),
            // A uid out of range with a valid gid, and the other way round.
            data("usr/bin/a", owned(1000, 0)),
            data("usr/bin/a", owned(0, 1000)),
            // Named owners whose ids are out of range.
            data(
                "usr/bin/a",
                TestFile::new("x").owner(Owner::named(1000, 1000, "user", "user")),
            ),
            // Directories, symlinks and hardlinks are checked too.
            data(
                "usr/share/doc/pkg",
                TestFile::directory().owner(Owner::new(1000, 1000, None, None)),
            ),
            data(
                "usr/bin/link",
                TestFile::symlink("a").owner(Owner::new(1000, 0, None, None)),
            ),
            data(
                "usr/bin/hard",
                TestFile::new("x")
                    .hardlink()
                    .owner(Owner::new(0, 1000, None, None)),
            ),
            // udebs are checked too.
            data("usr/bin/a", owned(1000, 0)).udeb(),
        ];
        BinaryPackageTester::new(WrongFileOwnerUidOrGid, pass, fail).test_and_snapshot();
    }
}
