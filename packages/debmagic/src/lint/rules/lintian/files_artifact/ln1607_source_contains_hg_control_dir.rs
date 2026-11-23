use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native_pedantic};

pub struct SourceContainsHgControlDir;

declare_rule! {
    /// A native package contains a Mercurial control directory.  A directory named
    /// `.hg`, at any depth.  Only for a native package (`3.0 (native)`, or `1.0`
    /// without an orig tarball). Pedantic, as in lintian. Extra is empty; the path
    /// is the Location. The non-native case is `debian-adds-hg-control-dir`.
    SourceContainsHgControlDir,
    code = "LN1607",
    tag = "source-contains-hg-control-dir",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsHgControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native_pedantic(ctx, Artifact::HgControlDir);
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
            native_tree(vec![file(".hg")]),
            native_tree(vec![dir(".hgtags")]),
            native_tree(vec![symlink(".hg")]),
        ];
        let fail = vec![
            native_tree(vec![dir(".hg")]),
            native_tree(vec![dir("src/.hg")]),
        ];
        SourceTreeTester::new(SourceContainsHgControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file(".hg")]),
            native(vec![changelog(), dir(".hgtags")]),
            native(vec![changelog(), symlink(".hg")]),
        ];
        let fail = vec![
            native(vec![changelog(), dir(".hg")]),
            native(vec![changelog(), dir("src/.hg")]),
        ];
        SourcePackageTester::new(SourceContainsHgControlDir, pass, fail).test_and_snapshot();
    }
}
