use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsHgControlDir;

declare_rule! {
    /// Debian added a Mercurial control directory (`.hg`).
    ///
    /// Directories only. `.hgtags` is a file Tag, not this directory. Extra
    /// is empty; the directory path with a trailing slash is the Location.
    DebianAddsHgControlDir,
    code = "LN0144",
    tag = "debian-adds-hg-control-dir",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsHgControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::HgControlDir);
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
            vec![control(), dir(".hgtags")],
            vec![control(), dir(".hgignore")],
            vec![control(), file(".hg")],
            vec![control(), symlink(".hg")],
        ];
        let fail = vec![
            vec![control(), dir(".hg")],
            vec![control(), dir("debian/.hg")],
            vec![control(), dir("src/.hg")],
            vec![control(), dir(".pc/.hg")],
        ];
        SourceTreeTester::new(DebianAddsHgControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![dir(".hg")]),
            in_orig(vec![dir(".hg")]),
            quilt(vec![dir(".pc/.hg")]),
            added(vec![dir("hg")]),
            added(vec![file(".hg")]),
        ];
        let fail = vec![
            added(vec![dir("debian/.hg")]),
            added(vec![dir(".hg")]),
            added(vec![dir(".pc/.hg")]),
        ];
        SourcePackageTester::new(DebianAddsHgControlDir, pass, fail).test_and_snapshot();
    }
}
