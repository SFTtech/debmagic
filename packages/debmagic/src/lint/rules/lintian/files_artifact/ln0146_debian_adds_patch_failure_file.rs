use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsPatchFailureFile;

declare_rule! {
    /// Debian added a failed-patch leftover.
    ///
    /// Regular files and hardlinks ending in `.orig` or `.rej`. The match is
    /// case-sensitive. Extra is empty; the path is the Location.
    DebianAddsPatchFailureFile,
    code = "LN0146",
    tag = "debian-adds-patch-failure-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsPatchFailureFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::PatchFailureFile);
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
            vec![control(), file("foo.original")],
            vec![control(), file("foo.orig.bak")],
            vec![control(), file("foo.ORIG")],
            vec![control(), file("foo.orig~")],
            vec![control(), dir("foo.orig")],
            vec![control(), symlink("foo.rej")],
        ];
        let fail = vec![
            vec![control(), file("foo.orig")],
            vec![control(), file("foo.rej")],
            vec![control(), file("debian/foo.rej")],
            vec![control(), hardlink("dir/x.rej")],
            vec![control(), file("foo~.orig")],
            vec![control(), file(".pc/foo.orig")],
        ];
        SourceTreeTester::new(DebianAddsPatchFailureFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![file("foo.orig")]),
            in_orig(vec![file("foo.orig")]),
            quilt(vec![file(".pc/foo.orig")]),
            added(vec![file("foo.original")]),
            added(vec![dir("foo.rej")]),
        ];
        let fail = vec![
            added(vec![file("debian/foo.rej")]),
            added(vec![file("foo.orig")]),
            added(vec![file(".pc/foo.orig")]),
        ];
        SourcePackageTester::new(DebianAddsPatchFailureFile, pass, fail).test_and_snapshot();
    }
}
