use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{Hierarchy, hierarchy_paths};

pub struct DirOrFileInRun;

declare_rule! {
    /// The package ships something the Filesystem Hierarchy Standard forbids here.
    ///
    /// Flags paths with `run/` followed by at least one more character (not for udebs). Lintian's `files/hierarchy/standard` Check is
    /// one chain of branches, so a path an earlier branch handles is not
    /// flagged here (see `hierarchy_standard.rs`). Extra is empty; the Data
    /// files path (with a trailing slash for directories) is the Location.
    DirOrFileInRun,
    code = "LN0044",
    tag = "dir-or-file-in-run",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DirOrFileInRun {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for path in hierarchy_paths(ctx, Hierarchy::Run) {
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
            tree(&[("run", true)]),
            tree(&[("other/run/x", false)]),
            tree(&[("run/x", false)]).udeb(),
        ];
        let fail = vec![
            tree(&[("run/x", false)]),
            tree(&[("run/x", true), ("run/x/file", false)]),
        ];
        BinaryPackageTester::new(DirOrFileInRun, pass, fail).test_and_snapshot();
    }
}
