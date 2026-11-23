use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native_pedantic};

pub struct SourceContainsSvnConflictFile;

declare_rule! {
    /// A native package contains an svn conflict file.  A regular file or hardlink
    /// named `<name>.r<revision>`, where the revision starts with a digit from 1 to
    /// 9.  Only for a native package (`3.0 (native)`, or `1.0` without an orig
    /// tarball). Pedantic, as in lintian. Extra is empty; the path is the Location.
    /// The non-native case is `debian-adds-svn-conflict-file`.
    SourceContainsSvnConflictFile,
    code = "LN1611",
    tag = "source-contains-svn-conflict-file",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsSvnConflictFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native_pedantic(ctx, Artifact::SvnConflictFile);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageTester, SourceTreeTester};

    use super::super::testing::{changelog, dir, file, hardlink, native, native_tree};
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            native_tree(vec![file("foo.r0")]),
            native_tree(vec![file("foo.r01")]),
            native_tree(vec![file(".r1")]),
            native_tree(vec![dir("foo.r1")]),
        ];
        let fail = vec![
            native_tree(vec![file("foo.r1")]),
            native_tree(vec![file("debian/foo.r1352")]),
            native_tree(vec![hardlink("a.r10")]),
        ];
        SourceTreeTester::new(SourceContainsSvnConflictFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file("foo.r0")]),
            native(vec![changelog(), file("foo.r01")]),
            native(vec![changelog(), file(".r1")]),
            native(vec![changelog(), dir("foo.r1")]),
        ];
        let fail = vec![
            native(vec![changelog(), file("foo.r1")]),
            native(vec![changelog(), file("debian/foo.r1352")]),
            native(vec![changelog(), hardlink("a.r10")]),
        ];
        SourcePackageTester::new(SourceContainsSvnConflictFile, pass, fail).test_and_snapshot();
    }
}
