use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native_pedantic};

pub struct SourceContainsBtsControlDir;

declare_rule! {
    /// A native package contains a bug tracker control directory.  A directory
    /// named `.be` or `.ditrack`, at any depth.  Only for a native package (`3.0
    /// (native)`, or `1.0` without an orig tarball). Pedantic, as in lintian. Extra
    /// is empty; the path is the Location. The non-native case is `debian-adds-bts-
    /// control-dir`.
    SourceContainsBtsControlDir,
    code = "LN1602",
    tag = "source-contains-bts-control-dir",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsBtsControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native_pedantic(ctx, Artifact::BtsControlDir);
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
            native_tree(vec![file(".be")]),
            native_tree(vec![symlink(".ditrack")]),
            native_tree(vec![dir("be")]),
        ];
        let fail = vec![
            native_tree(vec![dir(".be")]),
            native_tree(vec![dir(".ditrack")]),
            native_tree(vec![dir("debian/.be")]),
        ];
        SourceTreeTester::new(SourceContainsBtsControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file(".be")]),
            native(vec![changelog(), symlink(".ditrack")]),
            native(vec![changelog(), dir("be")]),
        ];
        let fail = vec![
            native(vec![changelog(), dir(".be")]),
            native(vec![changelog(), dir(".ditrack")]),
            native(vec![changelog(), dir("debian/.be")]),
        ];
        SourcePackageTester::new(SourceContainsBtsControlDir, pass, fail).test_and_snapshot();
    }
}
