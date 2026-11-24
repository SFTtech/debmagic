use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsEditorBackupFile;

declare_rule! {
    /// Debian added an editor backup file.
    ///
    /// Regular files and hardlinks whose path ends in `~`, or whose final
    /// component ends in `.swp`. `.swp` alone does not match. A name ending
    /// in `.orig` is a patch-failure file; that pattern is earlier. Extra is
    /// empty; the path is the Location.
    DebianAddsEditorBackupFile,
    code = "LN0142",
    tag = "debian-adds-editor-backup-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsEditorBackupFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::EditorBackupFile);
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
            vec![control(), file("foo.swo")],
            vec![control(), file(".swp")],
            vec![control(), file("foo.swp.bak")],
            vec![control(), file("foo~.orig")],
            vec![control(), dir("foo~")],
            vec![control(), symlink("foo~")],
        ];
        let fail = vec![
            vec![control(), file("foo~")],
            vec![control(), file("debian/foo~")],
            vec![control(), file("dir/a.swp")],
            vec![control(), hardlink("notes~")],
            vec![control(), file("foo.orig~")],
            vec![control(), file(".pc/foo~")],
        ];
        SourceTreeTester::new(DebianAddsEditorBackupFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![file("foo~")]),
            in_orig(vec![file("foo~")]),
            quilt(vec![file(".pc/foo~")]),
            added(vec![file("foo.swo")]),
            added(vec![dir("foo~")]),
        ];
        let fail = vec![
            added(vec![file("debian/foo~")]),
            added(vec![file("dir/a.swp")]),
            added(vec![file(".pc/foo~")]),
        ];
        SourcePackageTester::new(DebianAddsEditorBackupFile, pass, fail).test_and_snapshot();
    }
}
