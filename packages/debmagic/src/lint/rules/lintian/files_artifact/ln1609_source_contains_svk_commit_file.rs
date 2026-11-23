use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native_pedantic};

pub struct SourceContainsSvkCommitFile;

declare_rule! {
    /// A native package contains an svk commit file.  A regular file or hardlink
    /// named `svk-commit<one or more characters>.tmp`.  Only for a native package
    /// (`3.0 (native)`, or `1.0` without an orig tarball). Pedantic, as in lintian.
    /// Extra is empty; the path is the Location. The non-native case is `debian-
    /// adds-svk-commit-file`.
    SourceContainsSvkCommitFile,
    code = "LN1609",
    tag = "source-contains-svk-commit-file",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsSvkCommitFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native_pedantic(ctx, Artifact::SvkCommitFile);
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
            native_tree(vec![file("svk-commit.tmp")]),
            native_tree(vec![file("svk-commit444.txt")]),
            native_tree(vec![dir("svk-commit444.tmp")]),
        ];
        let fail = vec![
            native_tree(vec![file("svk-commit444.tmp")]),
            native_tree(vec![file("debian/svk-commit1.tmp")]),
        ];
        SourceTreeTester::new(SourceContainsSvkCommitFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file("svk-commit.tmp")]),
            native(vec![changelog(), file("svk-commit444.txt")]),
            native(vec![changelog(), dir("svk-commit444.tmp")]),
        ];
        let fail = vec![
            native(vec![changelog(), file("svk-commit444.tmp")]),
            native(vec![changelog(), file("debian/svk-commit1.tmp")]),
        ];
        SourcePackageTester::new(SourceContainsSvkCommitFile, pass, fail).test_and_snapshot();
    }
}
