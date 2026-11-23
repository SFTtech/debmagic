use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native_pedantic};

pub struct SourceContainsArchControlDir;

declare_rule! {
    /// A native package contains an Arch or baz control directory.  A directory
    /// named `{arch}` or `.arch-ids`, or matching `,,.+`, at any depth. A file or
    /// symlink with that name is not reported.  Only for a native package (`3.0
    /// (native)`, or `1.0` without an orig tarball). Pedantic, as in lintian. Extra
    /// is empty; the path is the Location. The non-native case is `debian-adds-
    /// arch-control-dir`.
    SourceContainsArchControlDir,
    code = "LN1600",
    tag = "source-contains-arch-control-dir",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsArchControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native_pedantic(ctx, Artifact::ArchControlDir);
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
            native_tree(vec![file("{arch}")]),
            native_tree(vec![dir("arch")]),
            native_tree(vec![symlink(".arch-ids")]),
        ];
        let fail = vec![
            native_tree(vec![dir("{arch}")]),
            native_tree(vec![dir(".arch-ids")]),
            native_tree(vec![dir(",,trace")]),
            native_tree(vec![dir("debian/{arch}")]),
        ];
        SourceTreeTester::new(SourceContainsArchControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file("{arch}")]),
            native(vec![changelog(), dir("arch")]),
            native(vec![changelog(), symlink(".arch-ids")]),
        ];
        let fail = vec![
            native(vec![changelog(), dir("{arch}")]),
            native(vec![changelog(), dir(".arch-ids")]),
            native(vec![changelog(), dir(",,trace")]),
            native(vec![changelog(), dir("debian/{arch}")]),
        ];
        SourcePackageTester::new(SourceContainsArchControlDir, pass, fail).test_and_snapshot();
    }
}
