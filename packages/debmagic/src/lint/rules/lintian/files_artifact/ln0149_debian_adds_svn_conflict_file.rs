use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsSvnConflictFile;

declare_rule! {
    /// Debian added a Subversion conflict file (`.rN`, `N >= 1`).
    ///
    /// Regular files and hardlinks. `foo.r0` and `foo.r01` do not match.
    /// `.#foo.1` is a CVS conflict copy. `.#foo.r1` ends in `.r1` and is this
    /// Tag. Extra is empty; the path is the Location.
    DebianAddsSvnConflictFile,
    code = "LN0149",
    tag = "debian-adds-svn-conflict-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsSvnConflictFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::SvnConflictFile);
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
            vec![control(), file("foo.r0")],
            vec![control(), file("foo.r")],
            vec![control(), file("foo.r01")],
            vec![control(), file(".r1")],
            vec![control(), dir("foo.r1")],
            vec![control(), symlink("foo.r1352")],
        ];
        let fail = vec![
            vec![control(), file("foo.r1")],
            vec![control(), file("foo.r1352")],
            vec![control(), file("debian/foo.r1352")],
            vec![control(), file("a.r10")],
            vec![control(), hardlink("dir/foo.r2")],
            vec![control(), file(".#foo.r1")],
            vec![control(), file(".pc/foo.r1352")],
        ];
        SourceTreeTester::new(DebianAddsSvnConflictFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![file("foo.r1")]),
            in_orig(vec![file("foo.r1")]),
            quilt(vec![file(".pc/foo.r1352")]),
            added(vec![file("foo.r0")]),
            added(vec![dir("foo.r1352")]),
        ];
        let fail = vec![
            added(vec![file("debian/foo.r1352")]),
            added(vec![file("foo.r1")]),
            added(vec![file(".#foo.r1")]),
            added(vec![file(".pc/foo.r1352")]),
        ];
        SourcePackageTester::new(DebianAddsSvnConflictFile, pass, fail).test_and_snapshot();
    }
}
