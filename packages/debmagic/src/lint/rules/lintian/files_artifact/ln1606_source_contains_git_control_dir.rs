use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native_pedantic};

pub struct SourceContainsGitControlDir;

declare_rule! {
    /// A native package contains a Git control directory.  A directory named
    /// `.git`, at any depth. `.github` is not reported.  Only for a native package
    /// (`3.0 (native)`, or `1.0` without an orig tarball). Pedantic, as in lintian.
    /// Extra is empty; the path is the Location. The non-native case is `debian-
    /// adds-git-control-dir`.
    SourceContainsGitControlDir,
    code = "LN1606",
    tag = "source-contains-git-control-dir",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsGitControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native_pedantic(ctx, Artifact::GitControlDir);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageTester, SourceTreeTester};

    use super::super::testing::{changelog, dir, file, native, native_tree, symlink};
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            native_tree(vec![file(".git")]),
            native_tree(vec![dir(".github")]),
            native_tree(vec![symlink(".git")]),
        ];
        let fail = vec![
            native_tree(vec![dir(".git")]),
            native_tree(vec![dir("debian/.git")]),
        ];
        SourceTreeTester::new(SourceContainsGitControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file(".git")]),
            native(vec![changelog(), dir(".github")]),
            native(vec![changelog(), symlink(".git")]),
        ];
        let fail = vec![
            native(vec![changelog(), dir(".git")]),
            native(vec![changelog(), dir("debian/.git")]),
        ];
        SourcePackageTester::new(SourceContainsGitControlDir, pass, fail).test_and_snapshot();
    }
}
