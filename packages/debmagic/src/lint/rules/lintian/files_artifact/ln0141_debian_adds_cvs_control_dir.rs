use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsCvsControlDir;

declare_rule! {
    /// Debian added a CVS control directory (`CVS`).
    ///
    /// The name is case-sensitive. Directories only. Extra is empty; the
    /// directory path with a trailing slash is the Location.
    DebianAddsCvsControlDir,
    code = "LN0141",
    tag = "debian-adds-cvs-control-dir",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsCvsControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::CvsControlDir);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageTester, SourceTreeTester};

    use super::super::testing::{added, control, dir, file, in_orig, native, quilt, symlink};
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            vec![control(), dir("cvs")],
            vec![control(), dir("CVSRoot")],
            vec![control(), file("CVS")],
            vec![control(), symlink("CVS")],
            vec![control(), file("debian/CVS/Entries")],
        ];
        let fail = vec![
            vec![control(), dir("CVS")],
            vec![control(), dir("debian/CVS")],
            vec![control(), dir("src/CVS")],
            vec![control(), dir(".pc/CVS")],
        ];
        SourceTreeTester::new(DebianAddsCvsControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![dir("CVS")]),
            in_orig(vec![dir("CVS")]),
            quilt(vec![dir(".pc/CVS")]),
            added(vec![dir("cvs")]),
            added(vec![file("CVS")]),
        ];
        let fail = vec![
            added(vec![dir("debian/CVS")]),
            added(vec![dir("CVS")]),
            added(vec![dir(".pc/CVS")]),
        ];
        SourcePackageTester::new(DebianAddsCvsControlDir, pass, fail).test_and_snapshot();
    }
}
