use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct ControlFileHasBadOwner;

declare_rule! {
    /// A recognised control file is not owned by `root/root` (or `0/0`).
    ///
    /// Checks the regular files of the Control files of a Binary package
    /// (deb and udeb) that `data/control-files/deb-permissions` (or
    /// `udeb-permissions` for a udeb) lists. Unknown names, hardlinks,
    /// directories and the `control` file itself are skipped, as lintian
    /// skips them before the owner check. Extra is the identity the file
    /// has, then `!= root/root (or 0/0)`. The Location is the control file.
    /// An Entry without an owner (a host tree, not a Binary package) is
    /// skipped: there is no identity to judge.
    ControlFileHasBadOwner,
    code = "LN1015",
    tag = "control-file-has-bad-owner",
    default_selected = true,
    experimental = false,
}

static DEB_FILES: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| names(include_str!("../../../data/control-files/deb-permissions")));
static UDEB_FILES: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| names(include_str!("../../../data/control-files/udeb-permissions")));

/// The first word of each line: the control file name, before its mode.
fn names(text: &'static str) -> HashSet<&'static str> {
    crate::lint::data::lines(text)
        .filter_map(|line| line.split_whitespace().next())
        .collect()
}

impl BinaryPackageRule for ControlFileHasBadOwner {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let known = if ctx.is_udeb() {
            &*UDEB_FILES
        } else {
            &*DEB_FILES
        };
        let Some(files) = ctx.control_files() else {
            return;
        };
        let reports: Vec<Diagnostic> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_regular_file())
            .filter(|entry| entry.path() != Path::new("control"))
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|name| known.contains(name))
            })
            .filter_map(|entry| {
                let identity = entry.owner()?.identity();
                (identity != "root/root" && identity != "0/0").then(|| {
                    Diagnostic::error(format!("{identity} != root/root (or 0/0)")).with_location(
                        Location {
                            path: entry.path().to_path_buf(),
                            line: None,
                            column: None,
                        },
                    )
                })
            })
            .collect();
        for diagnostic in reports {
            ctx.diagnostic(diagnostic);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(name: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(name, file)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            case("md5sums", TestFile::new("").owned_by("root", "root")),
            case(
                "control",
                TestFile::new("Package: x\n").owned_by("nobody", "root"),
            ),
            case("unknown", TestFile::new("").owned_by("nobody", "root")),
            case("md5sums", TestFile::new("")),
            case(
                "md5sums",
                TestFile::new("").owned_by("nobody", "root").hardlink(),
            ),
            case("postinst", TestFile::new("").owned_by("root", "root")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .udeb()
                .control_file("md5sums", TestFile::new("").owned_by("root", "root")),
            // `udeb-permissions` does not list `md5sums`.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .udeb()
                .control_file("md5sums", TestFile::new("").owned_by("nobody", "root")),
        ];
        let fail = vec![
            case("md5sums", TestFile::new("").owned_by("nobody", "root")),
            case("postinst", TestFile::new("").owned_by("root", "games")),
            case("md5sums", TestFile::new("").owned_by("alice", "alice")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .udeb()
                .control_file("postinst", TestFile::new("").owned_by("nobody", "root")),
        ];
        BinaryPackageTester::new(ControlFileHasBadOwner, pass, fail).test_and_snapshot();
    }
}
