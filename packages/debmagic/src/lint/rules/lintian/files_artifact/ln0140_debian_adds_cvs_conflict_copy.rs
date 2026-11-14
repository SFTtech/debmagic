use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsCvsConflictCopy;

declare_rule! {
    /// Debian added a CVS conflict copy (`.#name.N`).
    ///
    /// Regular files and hardlinks. The revision may be several dotted
    /// numbers (`.#foo.1.1`). `.#foo.r1` ends in `.r1`, so it is a Subversion
    /// conflict file. Extra is empty; the path is the Location.
    DebianAddsCvsConflictCopy,
    code = "LN0140",
    tag = "debian-adds-cvs-conflict-copy",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsCvsConflictCopy {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::CvsConflictCopy);
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
            vec![control(), file(".#foo")],
            vec![control(), file(".#.1")],
            vec![control(), file("foo.#bar.1")],
            vec![control(), file(".#foo.1a")],
            vec![control(), dir(".#foo.1")],
            vec![control(), symlink(".#foo.1")],
            vec![control(), file(".#foo.r1")],
        ];
        let fail = vec![
            vec![control(), file(".#foo.1")],
            vec![control(), file("debian/.#foo.1.1")],
            vec![control(), file(".#foo.0")],
            vec![control(), hardlink(".#bar.12")],
            vec![control(), file(".pc/.#foo.1")],
        ];
        SourceTreeTester::new(DebianAddsCvsConflictCopy, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![file(".#foo.1")]),
            in_orig(vec![file(".#foo.1")]),
            quilt(vec![file(".pc/.#foo.1")]),
            added(vec![file(".#foo")]),
            added(vec![dir(".#foo.1")]),
        ];
        let fail = vec![
            added(vec![file("debian/.#foo.1.1")]),
            added(vec![file(".#foo.1")]),
            added(vec![file(".pc/.#foo.1")]),
        ];
        SourcePackageTester::new(DebianAddsCvsConflictCopy, pass, fail).test_and_snapshot();
    }
}
