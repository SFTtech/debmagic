use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native_pedantic};

pub struct SourceContainsBzrControlDir;

declare_rule! {
    /// A native package contains a Bazaar control directory.  A directory named
    /// `.bzr`, at any depth.  Only for a native package (`3.0 (native)`, or `1.0`
    /// without an orig tarball). Pedantic, as in lintian. Extra is empty; the path
    /// is the Location. The non-native case is `debian-adds-bzr-control-dir`.
    SourceContainsBzrControlDir,
    code = "LN1603",
    tag = "source-contains-bzr-control-dir",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsBzrControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native_pedantic(ctx, Artifact::BzrControlDir);
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
            native_tree(vec![file(".bzr")]),
            native_tree(vec![dir("bzr")]),
            native_tree(vec![symlink(".bzr")]),
        ];
        let fail = vec![
            native_tree(vec![dir(".bzr")]),
            native_tree(vec![dir("src/.bzr")]),
        ];
        SourceTreeTester::new(SourceContainsBzrControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file(".bzr")]),
            native(vec![changelog(), dir("bzr")]),
            native(vec![changelog(), symlink(".bzr")]),
        ];
        let fail = vec![
            native(vec![changelog(), dir(".bzr")]),
            native(vec![changelog(), dir("src/.bzr")]),
        ];
        SourcePackageTester::new(SourceContainsBzrControlDir, pass, fail).test_and_snapshot();
    }
}
