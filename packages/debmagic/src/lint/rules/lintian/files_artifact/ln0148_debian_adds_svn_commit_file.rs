use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsSvnCommitFile;

declare_rule! {
    /// Debian added a Subversion commit temporary file.
    ///
    /// Regular files and hardlinks named `svn-commit.tmp` or
    /// `svn-commit.<words>.tmp`. Extra is empty; the path is the Location.
    DebianAddsSvnCommitFile,
    code = "LN0148",
    tag = "debian-adds-svn-commit-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsSvnCommitFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::SvnCommitFile);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageTester, SourceTreeTester};

    use super::super::testing::{
        added, control, dir, file, hardlink, in_orig, native, quilt, symlink,
    };
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            vec![control(), file("svk-commit444.tmp")],
            vec![control(), file("svn-commit.tmp.bak")],
            vec![control(), file("Xsvn-commit.tmp")],
            vec![control(), dir("svn-commit.tmp")],
            vec![control(), symlink("svn-commit.tmp")],
        ];
        let fail = vec![
            vec![control(), file("svn-commit.tmp")],
            vec![control(), file("debian/svn-commit.tmp")],
            vec![control(), file("svn-commit.msg.tmp")],
            vec![control(), hardlink("dir/svn-commit.1.tmp")],
            vec![control(), file(".pc/svn-commit.tmp")],
        ];
        SourceTreeTester::new(DebianAddsSvnCommitFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![file("svn-commit.tmp")]),
            in_orig(vec![file("svn-commit.tmp")]),
            quilt(vec![file(".pc/svn-commit.tmp")]),
            added(vec![file("svn-commit.txt")]),
            added(vec![dir("svn-commit.tmp")]),
        ];
        let fail = vec![
            added(vec![file("debian/svn-commit.tmp")]),
            added(vec![file("svn-commit.msg.tmp")]),
            added(vec![file(".pc/svn-commit.tmp")]),
        ];
        SourcePackageTester::new(DebianAddsSvnCommitFile, pass, fail).test_and_snapshot();
    }
}
