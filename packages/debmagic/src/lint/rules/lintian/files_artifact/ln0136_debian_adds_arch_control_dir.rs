use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsArchControlDir;

declare_rule! {
    /// Debian added an Arch or baz control directory.
    ///
    /// Directories named `{arch}` or `.arch-ids`, or matching `,,.+`. Any
    /// depth. A file or symlink with that name is left alone. Extra is empty;
    /// the directory path with a trailing slash is the Location.
    DebianAddsArchControlDir,
    code = "LN0136",
    tag = "debian-adds-arch-control-dir",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsArchControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::ArchControlDir);
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
            vec![control(), file("{arch}")],
            vec![control(), dir("arch")],
            vec![control(), symlink(".arch-ids")],
            vec![control(), file("debian/{arch}/config")],
        ];
        let fail = vec![
            vec![control(), dir("{arch}")],
            vec![control(), dir(".arch-ids")],
            vec![control(), dir(",,trace")],
            vec![control(), dir("debian/{arch}")],
            vec![control(), dir("src/.arch-ids")],
            vec![control(), dir(",,")],
            vec![control(), dir(",,.git")],
            vec![control(), dir(".pc/{arch}")],
        ];
        SourceTreeTester::new(DebianAddsArchControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![dir("{arch}")]),
            in_orig(vec![dir("{arch}")]),
            quilt(vec![dir(".pc/{arch}")]),
            added(vec![dir("arch")]),
            added(vec![file("{arch}")]),
        ];
        let fail = vec![
            added(vec![dir("debian/{arch}")]),
            added(vec![dir(".arch-ids")]),
            added(vec![dir("pkg/,,debug")]),
            added(vec![dir(".pc/{arch}")]),
        ];
        SourcePackageTester::new(DebianAddsArchControlDir, pass, fail).test_and_snapshot();
    }
}
