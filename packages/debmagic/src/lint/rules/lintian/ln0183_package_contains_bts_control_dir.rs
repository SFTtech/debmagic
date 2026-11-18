use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsBtsControlDir;

declare_rule! {
    /// The Binary package ships a bug-tracker control directory.
    ///
    /// Walks Data files directories whose path matches `/\.(?:be|ditrack)/?$`
    /// (a `.be` or `.ditrack` directory under some parent). Udebs are checked
    /// too. Extra is empty; the path, with a trailing slash, is the Location.
    PackageContainsBtsControlDir,
    code = "LN0183",
    tag = "package-contains-bts-control-dir",
    default_selected = true,
    experimental = false,
}

static BTS_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/\.(?:be|ditrack)/?$").expect("valid regex"));

impl BinaryPackageRule for PackageContainsBtsControlDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::Directory])
            .path(&BTS_DIR)
            .into_iter()
            .map(|entry| PathBuf::from(format!("{}/", entry.path().display())))
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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
            // A file with that name is not a directory.
            tree(&[("usr/share/foo/.be", false)]),
            tree(&[("usr/lib/perl/.ditrack", false)]),
            // The name must be exactly `.be` or `.ditrack`.
            tree(&[
                ("usr/share/foo/.bes", true),
                ("usr/share/foo/.ditracker", true),
                ("usr/share/foo/.BE", true),
            ]),
            // At the root there is no slash before the dot.
            tree(&[(".be", true), (".ditrack", true)]),
            // A child of the directory is not the directory.
            tree(&[("usr/share/foo/.be/tracked", false)]),
            tree(&[("usr/share/foo/bugs", true)]),
        ];
        let fail = vec![
            tree(&[("usr/share/files-package-contains-foo/.be", true)]),
            tree(&[("usr/lib/ma-dir/perl/version/.ditrack", true)]),
            tree(&[
                ("usr/lib/ma-dir/perl/version/.be", true),
                ("usr/lib/ma-dir/perl/version/.ditrack", true),
            ]),
            tree(&[("usr/share/example/.be", true)]).udeb(),
        ];
        BinaryPackageTester::new(PackageContainsBtsControlDir, pass, fail).test_and_snapshot();
    }
}
