use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native_pedantic};

pub struct SourceContainsSvnControlDir;

declare_rule! {
    /// A native package contains an svn control directory.  A directory named
    /// `.svn`, at any depth.  Only for a native package (`3.0 (native)`, or `1.0`
    /// without an orig tarball). Pedantic, as in lintian. Extra is empty; the path
    /// is the Location. The non-native case is `debian-adds-svn-control-dir`.
    SourceContainsSvnControlDir,
    code = "LN1612",
    tag = "source-contains-svn-control-dir",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsSvnControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native_pedantic(ctx, Artifact::SvnControlDir);
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
            native_tree(vec![file(".svn")]),
            native_tree(vec![dir("svn")]),
            native_tree(vec![symlink(".svn")]),
        ];
        let fail = vec![
            native_tree(vec![dir(".svn")]),
            native_tree(vec![dir("debian/.svn")]),
        ];
        SourceTreeTester::new(SourceContainsSvnControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file(".svn")]),
            native(vec![changelog(), dir("svn")]),
            native(vec![changelog(), symlink(".svn")]),
        ];
        let fail = vec![
            native(vec![changelog(), dir(".svn")]),
            native(vec![changelog(), dir("debian/.svn")]),
        ];
        SourcePackageTester::new(SourceContainsSvnControlDir, pass, fail).test_and_snapshot();
    }
}
