use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsGitControlDir;

declare_rule! {
    /// Debian added a Git control directory (`.git`).
    ///
    /// Directories only. `.github` does not match. Extra is empty; the
    /// directory path with a trailing slash is the Location.
    DebianAddsGitControlDir,
    code = "LN0143",
    tag = "debian-adds-git-control-dir",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsGitControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::GitControlDir);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester};

    use super::super::testing::{
        added, control, dir, file, in_orig, native, quilt, quilt_dsc, symlink, v1_dsc,
    };
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            vec![control(), dir(".github")],
            vec![control(), file(".git")],
            vec![control(), symlink(".git")],
            vec![control(), file("debian/.git/config")],
            vec![control(), dir(",,.git")],
        ];
        let fail = vec![
            vec![control(), dir(".git")],
            vec![control(), dir("debian/.git")],
            vec![control(), dir("src/.git")],
            vec![control(), dir(".pc/.git")],
        ];
        SourceTreeTester::new(DebianAddsGitControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![dir(".git")]),
            SourcePackageCase::from(vec![dir(".git")]).dsc(v1_dsc()),
            in_orig(vec![dir(".git")]),
            quilt(vec![dir(".pc/.git")]),
            added(vec![dir("git")]),
            added(vec![file(".git")]),
        ];
        let fail = vec![
            added(vec![dir("debian/.git")]),
            added(vec![dir(".git")]),
            added(vec![dir(".pc/.git")]),
            SourcePackageCase::from(vec![dir(".git")]).dsc(quilt_dsc()),
        ];
        SourcePackageTester::new(DebianAddsGitControlDir, pass, fail).test_and_snapshot();
    }
}
