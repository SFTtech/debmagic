use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native_pedantic};

pub struct SourceContainsSvnCommitFile;

declare_rule! {
    /// A native package contains an svn commit file.  A regular file or hardlink
    /// named `svn-commit.tmp` or `svn-commit.<words>.tmp`.  Only for a native
    /// package (`3.0 (native)`, or `1.0` without an orig tarball). Pedantic, as in
    /// lintian. Extra is empty; the path is the Location. The non-native case is
    /// `debian-adds-svn-commit-file`.
    SourceContainsSvnCommitFile,
    code = "LN1610",
    tag = "source-contains-svn-commit-file",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsSvnCommitFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native_pedantic(ctx, Artifact::SvnCommitFile);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageTester, SourceTreeTester};

    use super::super::testing::{changelog, dir, file, native, native_tree};
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            native_tree(vec![file("svn-commit.msg")]),
            native_tree(vec![dir("svn-commit.tmp")]),
        ];
        let fail = vec![
            native_tree(vec![file("svn-commit.tmp")]),
            native_tree(vec![file("debian/svn-commit.msg.tmp")]),
        ];
        SourceTreeTester::new(SourceContainsSvnCommitFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file("svn-commit.msg")]),
            native(vec![changelog(), dir("svn-commit.tmp")]),
        ];
        let fail = vec![
            native(vec![changelog(), file("svn-commit.tmp")]),
            native(vec![changelog(), file("debian/svn-commit.msg.tmp")]),
        ];
        SourcePackageTester::new(SourceContainsSvnCommitFile, pass, fail).test_and_snapshot();
    }
}
