use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, follow_symlinks};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct ServiceFileIsNotAFile;

declare_rule! {
    /// A `*.service` unit under a `systemd/system/` directory is not a readable
    /// file: a directory, a dangling or escaping symlink, or a device or pipe.
    /// A symlink to `/dev/null` (a masked unit) is fine. Extra is empty; the unit
    /// path is the Location. Udebs are checked too.
    ServiceFileIsNotAFile,
    code = "LN1229",
    tag = "service-file-is-not-a-file",
    default_selected = true,
    experimental = false,
}

static SERVICE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/systemd/system/.*\.service$").expect("valid regex"));

impl BinaryPackageRule for ServiceFileIsNotAFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut paths = Vec::new();
        for entry in files.entries() {
            // lintian's names of directories end in `/`, which never matches `.service$`.
            if entry.is_dir() || !SERVICE.is_match(&entry.path().to_string_lossy()) {
                continue;
            }
            let masked = entry.is_symlink() && entry.link_target() == Some("/dev/null".as_ref());
            if masked || follow_symlinks(files, entry).is_some() {
                continue;
            }
            paths.push(entry.path().to_path_buf());
        }
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

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![(
                "lib/systemd/system/foo.service",
                TestFile::new("[Unit]\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/system/foo.service",
                TestFile::new("[Unit]\n").hardlink(),
            )]),
            // A masked unit.
            BinaryPackageCase::from(vec![(
                "etc/systemd/system/foo.service",
                TestFile::symlink("/dev/null"),
            )]),
            // A symlink to a readable unit in the tree.
            BinaryPackageCase::from(vec![
                (
                    "lib/systemd/system/foo.service",
                    TestFile::symlink("bar.service"),
                ),
                ("lib/systemd/system/bar.service", TestFile::new("[Unit]\n")),
            ]),
            // lintian's names of directories end in `/`, so a directory is never a hit.
            BinaryPackageCase::from(vec![(
                "lib/systemd/system/foo.service",
                TestFile::directory(),
            )]),
            // Not a unit file.
            BinaryPackageCase::from(vec![(
                "lib/systemd/system/foo.socket",
                TestFile::symlink("/nowhere"),
            )]),
            // Outside a systemd/system directory.
            BinaryPackageCase::from(vec![(
                "lib/systemd/foo.service",
                TestFile::symlink("/nowhere"),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "lib/systemd/system/foo.service",
                TestFile::symlink("missing.service"),
            )]),
            BinaryPackageCase::from(vec![(
                "etc/systemd/system/foo.service",
                TestFile::symlink("/usr/lib/systemd/system/foo.service"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/system/fifo-pipe.service",
                TestFile::fifo(),
            )]),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![(
                "lib/systemd/system/foo.service",
                TestFile::symlink("missing.service"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(ServiceFileIsNotAFile, pass, fail).test_and_snapshot();
    }
}
