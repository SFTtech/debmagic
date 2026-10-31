use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsBzrControlDir;

declare_rule! {
    /// Debian added a Bazaar control directory (`.bzr`).
    ///
    /// Directories only, at any depth. Extra is empty; the directory path
    /// with a trailing slash is the Location.
    DebianAddsBzrControlDir,
    code = "LN0139",
    tag = "debian-adds-bzr-control-dir",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsBzrControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::BzrControlDir);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageTester, SourceTreeTester};

    use super::super::testing::{added, control, dir, file, in_orig, native, quilt, symlink};
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            vec![control(), dir(".bzr-backup")],
            vec![control(), file(".bzr")],
            vec![control(), symlink(".bzr")],
        ];
        let fail = vec![
            vec![control(), dir(".bzr")],
            vec![control(), dir("debian/.bzr")],
            vec![control(), dir("src/.bzr")],
            vec![control(), dir(".pc/.bzr")],
        ];
        SourceTreeTester::new(DebianAddsBzrControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![dir(".bzr")]),
            in_orig(vec![dir(".bzr")]),
            quilt(vec![dir(".pc/.bzr")]),
            added(vec![dir("bzr")]),
            added(vec![file(".bzr")]),
        ];
        let fail = vec![
            added(vec![dir("debian/.bzr")]),
            added(vec![dir(".bzr")]),
            added(vec![dir(".pc/.bzr")]),
        ];
        SourcePackageTester::new(DebianAddsBzrControlDir, pass, fail).test_and_snapshot();
    }
}
