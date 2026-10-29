use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{Hierarchy, hierarchy_paths};

pub struct SubdirInUsrBin;

declare_rule! {
    /// A directory sits in `usr/bin`.
    ///
    /// Follows lintian's `files/hierarchy/standard` branch order (see
    /// `hierarchy_standard.rs`). Extra is empty; the Data files path (with a
    /// trailing slash for directories) is the Location. Udebs are checked too.
    SubdirInUsrBin,
    code = "LN0049",
    tag = "subdir-in-usr-bin",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SubdirInUsrBin {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for path in hierarchy_paths(ctx, Hierarchy::SubdirInUsrBin) {
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

    fn tree(entries: &[(&str, TestFile)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let file = || TestFile::new("x");
        let dir = TestFile::directory;
        let pass = vec![
            tree(&[]),
            tree(&[("usr/bin", dir()), ("usr/bin/tool", file())]),
            tree(&[("usr/bin/X11", dir()), ("usr/bin/mh", dir())]),
            tree(&[("usr/bin/X11/sub", dir())]),
            tree(&[("usr/bin/tool", TestFile::symlink("x"))]),
            tree(&[("usr/sbin/sub", dir())]),
        ];
        let fail = vec![
            tree(&[("usr/bin/foo", dir())]),
            tree(&[("usr/bin/bin", dir()), ("usr/bin/bin/x", file())]),
            tree(&[("usr/bin/mhx", dir())]),
            tree(&[("usr/bin/foo", dir())]).udeb(),
        ];
        BinaryPackageTester::new(SubdirInUsrBin, pass, fail).test_and_snapshot();
    }
}
