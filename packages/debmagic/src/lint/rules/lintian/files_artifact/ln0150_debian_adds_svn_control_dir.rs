use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsSvnControlDir;

declare_rule! {
    /// Debian added a Subversion control directory (`.svn`).
    ///
    /// Directories only, at any depth. Extra is empty; the directory path
    /// with a trailing slash is the Location.
    DebianAddsSvnControlDir,
    code = "LN0150",
    tag = "debian-adds-svn-control-dir",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsSvnControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::SvnControlDir);
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
            vec![control(), dir(".svnignore")],
            vec![control(), file(".svn")],
            vec![control(), symlink(".svn")],
        ];
        let fail = vec![
            vec![control(), dir(".svn")],
            vec![control(), dir("debian/.svn")],
            vec![control(), dir("src/.svn")],
            vec![control(), dir(".pc/.svn")],
        ];
        SourceTreeTester::new(DebianAddsSvnControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![dir(".svn")]),
            in_orig(vec![dir(".svn")]),
            quilt(vec![dir(".pc/.svn")]),
            added(vec![dir("svn")]),
            added(vec![file(".svn")]),
        ];
        let fail = vec![
            added(vec![dir("debian/.svn")]),
            added(vec![dir(".svn")]),
            added(vec![dir(".pc/.svn")]),
        ];
        SourcePackageTester::new(DebianAddsSvnControlDir, pass, fail).test_and_snapshot();
    }
}
