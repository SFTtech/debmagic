use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsHgTagsFile;

declare_rule! {
    /// Debian added a Mercurial tags file (`.hgtags`).
    ///
    /// Regular files and hardlinks. Extra is empty; the path is the Location.
    DebianAddsHgTagsFile,
    code = "LN0145",
    tag = "debian-adds-hg-tags-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsHgTagsFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::HgTagsFile);
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
            vec![control(), file("hgtags")],
            vec![control(), file(".hgtags.bak")],
            vec![control(), dir(".hgtags")],
            vec![control(), symlink(".hgtags")],
        ];
        let fail = vec![
            vec![control(), file(".hgtags")],
            vec![control(), file("debian/.hgtags")],
            vec![control(), hardlink("src/.hgtags")],
            vec![control(), file(".pc/.hgtags")],
        ];
        SourceTreeTester::new(DebianAddsHgTagsFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![file(".hgtags")]),
            in_orig(vec![file(".hgtags")]),
            quilt(vec![file(".pc/.hgtags")]),
            added(vec![file("hgtags")]),
            added(vec![dir(".hgtags")]),
        ];
        let fail = vec![
            added(vec![file("debian/.hgtags")]),
            added(vec![file(".hgtags")]),
            added(vec![file(".pc/.hgtags")]),
        ];
        SourcePackageTester::new(DebianAddsHgTagsFile, pass, fail).test_and_snapshot();
    }
}
