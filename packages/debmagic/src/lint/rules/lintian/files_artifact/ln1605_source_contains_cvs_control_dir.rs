use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native_pedantic};

pub struct SourceContainsCvsControlDir;

declare_rule! {
    /// A native package contains a CVS control directory.  A directory named `CVS`,
    /// at any depth. A lower-case `cvs` is not reported.  Only for a native package
    /// (`3.0 (native)`, or `1.0` without an orig tarball). Pedantic, as in lintian.
    /// Extra is empty; the path is the Location. The non-native case is `debian-
    /// adds-cvs-control-dir`.
    SourceContainsCvsControlDir,
    code = "LN1605",
    tag = "source-contains-cvs-control-dir",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsCvsControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native_pedantic(ctx, Artifact::CvsControlDir);
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
            native_tree(vec![file("CVS")]),
            native_tree(vec![dir("cvs")]),
            native_tree(vec![symlink("CVS")]),
        ];
        let fail = vec![
            native_tree(vec![dir("CVS")]),
            native_tree(vec![dir("debian/CVS")]),
        ];
        SourceTreeTester::new(SourceContainsCvsControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file("CVS")]),
            native(vec![changelog(), dir("cvs")]),
            native(vec![changelog(), symlink("CVS")]),
        ];
        let fail = vec![
            native(vec![changelog(), dir("CVS")]),
            native(vec![changelog(), dir("debian/CVS")]),
        ];
        SourcePackageTester::new(SourceContainsCvsControlDir, pass, fail).test_and_snapshot();
    }
}
