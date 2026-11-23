use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native_pedantic};

pub struct SourceContainsCvsConflictCopy;

declare_rule! {
    /// A native package contains a CVS conflict copy.  A regular file or hardlink
    /// named `.#<name>.<digits>` with at least one `.<digits>` group and a non-
    /// empty name.  Only for a native package (`3.0 (native)`, or `1.0` without an
    /// orig tarball). Pedantic, as in lintian. Extra is empty; the path is the
    /// Location. The non-native case is `debian-adds-cvs-conflict-copy`.
    SourceContainsCvsConflictCopy,
    code = "LN1604",
    tag = "source-contains-cvs-conflict-copy",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsCvsConflictCopy {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native_pedantic(ctx, Artifact::CvsConflictCopy);
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
            native_tree(vec![file(".#foo")]),
            native_tree(vec![file(".#.1")]),
            native_tree(vec![dir(".#foo.1")]),
            native_tree(vec![symlink(".#foo.1")]),
        ];
        let fail = vec![
            native_tree(vec![file(".#foo.1")]),
            native_tree(vec![file("debian/.#foo.1.1")]),
            native_tree(vec![hardlink(".#foo.1")]),
        ];
        SourceTreeTester::new(SourceContainsCvsConflictCopy, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file(".#foo")]),
            native(vec![changelog(), file(".#.1")]),
            native(vec![changelog(), dir(".#foo.1")]),
            native(vec![changelog(), symlink(".#foo.1")]),
        ];
        let fail = vec![
            native(vec![changelog(), file(".#foo.1")]),
            native(vec![changelog(), file("debian/.#foo.1.1")]),
            native(vec![changelog(), hardlink(".#foo.1")]),
        ];
        SourcePackageTester::new(SourceContainsCvsConflictCopy, pass, fail).test_and_snapshot();
    }
}
