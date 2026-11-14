use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsBtsControlDir;

declare_rule! {
    /// Debian added a bugs-everywhere or ditrack control directory.
    ///
    /// Directories named `.be` or `.ditrack`. `.before` does not match. Extra
    /// is empty; the directory path with a trailing slash is the Location.
    DebianAddsBtsControlDir,
    code = "LN0138",
    tag = "debian-adds-bts-control-dir",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsBtsControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::BtsControlDir);
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
            vec![control(), dir(".before")],
            vec![control(), file(".be")],
            vec![control(), file(".ditrack")],
            vec![control(), symlink(".be")],
        ];
        let fail = vec![
            vec![control(), dir(".be")],
            vec![control(), dir(".ditrack")],
            vec![control(), dir("debian/.be")],
            vec![control(), dir("src/.ditrack")],
            vec![control(), dir(".pc/.be")],
        ];
        SourceTreeTester::new(DebianAddsBtsControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![dir(".be")]),
            in_orig(vec![dir(".be")]),
            quilt(vec![dir(".pc/.be")]),
            added(vec![dir("be")]),
            added(vec![file(".ditrack")]),
        ];
        let fail = vec![
            added(vec![dir("debian/.be")]),
            added(vec![dir(".ditrack")]),
            added(vec![dir(".pc/.be")]),
        ];
        SourcePackageTester::new(DebianAddsBtsControlDir, pass, fail).test_and_snapshot();
    }
}
