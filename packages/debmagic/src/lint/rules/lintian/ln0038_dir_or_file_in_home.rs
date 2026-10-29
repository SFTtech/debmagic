use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{Hierarchy, hierarchy_paths};

pub struct DirOrFileInHome;

declare_rule! {
    /// The package ships something the Filesystem Hierarchy Standard forbids here.
    ///
    /// Flags paths with `home/` or `root/` followed by at least one more character. Lintian's `files/hierarchy/standard` Check is
    /// one chain of branches, so a path an earlier branch handles is not
    /// flagged here (see `hierarchy_standard.rs`). Extra is empty; the Data
    /// files path (with a trailing slash for directories) is the Location.
    DirOrFileInHome,
    code = "LN0038",
    tag = "dir-or-file-in-home",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DirOrFileInHome {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for path in hierarchy_paths(ctx, Hierarchy::Home) {
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
            tree(&[("home", true)]),
            tree(&[("other/home/user", false)]),
        ];
        let fail = vec![
            tree(&[("home/user", false)]),
            tree(&[("home/user", true), ("home/user/file", false)]),
            tree(&[("root/x", false)]),
        ];
        BinaryPackageTester::new(DirOrFileInHome, pass, fail).test_and_snapshot();
    }
}
