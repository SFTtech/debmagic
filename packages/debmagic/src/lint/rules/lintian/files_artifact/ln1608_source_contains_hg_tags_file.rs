use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native_pedantic};

pub struct SourceContainsHgTagsFile;

declare_rule! {
    /// A native package contains a Mercurial tags file.  A regular file or hardlink
    /// named `.hgtags`, at any depth.  Only for a native package (`3.0 (native)`,
    /// or `1.0` without an orig tarball). Pedantic, as in lintian. Extra is empty;
    /// the path is the Location. The non-native case is `debian-adds-hg-tags-file`.
    SourceContainsHgTagsFile,
    code = "LN1608",
    tag = "source-contains-hg-tags-file",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsHgTagsFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native_pedantic(ctx, Artifact::HgTagsFile);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageTester, SourceTreeTester};

    use super::super::testing::{changelog, dir, file, hardlink, native, native_tree, symlink};
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            native_tree(vec![dir(".hgtags")]),
            native_tree(vec![file(".hgtags.bak")]),
            native_tree(vec![symlink(".hgtags")]),
        ];
        let fail = vec![
            native_tree(vec![file(".hgtags")]),
            native_tree(vec![file("debian/.hgtags")]),
            native_tree(vec![hardlink(".hgtags")]),
        ];
        SourceTreeTester::new(SourceContainsHgTagsFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), dir(".hgtags")]),
            native(vec![changelog(), file(".hgtags.bak")]),
            native(vec![changelog(), symlink(".hgtags")]),
        ];
        let fail = vec![
            native(vec![changelog(), file(".hgtags")]),
            native(vec![changelog(), file("debian/.hgtags")]),
            native(vec![changelog(), hardlink(".hgtags")]),
        ];
        SourcePackageTester::new(SourceContainsHgTagsFile, pass, fail).test_and_snapshot();
    }
}
