use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsSvkCommitFile;

declare_rule! {
    /// Debian added an svk commit temporary file.
    ///
    /// Regular files and hardlinks matching `svk-commit`, then at least one
    /// character, then `.tmp` (`svk-commit444.tmp`). `svk-commit.tmp` does
    /// not match. Extra is empty; the path is the Location.
    DebianAddsSvkCommitFile,
    code = "LN0147",
    tag = "debian-adds-svk-commit-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsSvkCommitFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::SvkCommitFile);
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
            vec![control(), file("svk-commit.tmp")],
            vec![control(), file("svn-commit.tmp")],
            vec![control(), file("svk-commit444.tmp.bak")],
            vec![control(), file("mysvk-commit444.tmp")],
            vec![control(), dir("svk-commit444.tmp")],
            vec![control(), symlink("svk-commit444.tmp")],
        ];
        let fail = vec![
            vec![control(), file("svk-commit444.tmp")],
            vec![control(), file("debian/svk-commit444.tmp")],
            vec![control(), file("svk-commit.x.tmp")],
            vec![control(), hardlink("dir/svk-commit9.tmp")],
            vec![control(), file(".pc/svk-commit444.tmp")],
        ];
        SourceTreeTester::new(DebianAddsSvkCommitFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![file("svk-commit444.tmp")]),
            in_orig(vec![file("svk-commit444.tmp")]),
            quilt(vec![file(".pc/svk-commit444.tmp")]),
            added(vec![file("svk-commit.tmp")]),
            added(vec![dir("svk-commit444.tmp")]),
        ];
        let fail = vec![
            added(vec![file("debian/svk-commit444.tmp")]),
            added(vec![file("svk-commit444.tmp")]),
            added(vec![file(".pc/svk-commit444.tmp")]),
        ];
        SourcePackageTester::new(DebianAddsSvkCommitFile, pass, fail).test_and_snapshot();
    }
}
