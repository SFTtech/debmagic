use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native};

pub struct SourceContainsEditorBackupFile;

declare_rule! {
    /// A native package contains an editor backup file.
    ///
    /// Only for a native package (`3.0 (native)`, or `1.0` without an orig
    /// tarball). Regular files and hardlinks whose path ends in `~`, or whose
    /// final component ends in `.swp`. Every patched path counts, including
    /// under `.pc/`. Extra is empty; the path is the Location. The non-native
    /// case is `debian-adds-editor-backup-file`.
    SourceContainsEditorBackupFile,
    code = "LN1244",
    tag = "source-contains-editor-backup-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for SourceContainsEditorBackupFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native(ctx, Artifact::EditorBackupFile);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageTester, SourceTreeTester, TestFile};

    use super::super::testing::{control, dir, file, hardlink, native, symlink};
    use super::*;

    /// Native packages need a changelog with a Version that has no revision.
    fn changelog() -> (String, TestFile) {
        (
            "debian/changelog".to_string(),
            TestFile::new(
                "example (1.0) unstable; urgency=medium\n\n  * Initial.\n\n \
                 -- Jane <jane@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n",
            ),
        )
    }

    fn tree(mut extra: Vec<(String, TestFile)>) -> Vec<(String, TestFile)> {
        extra.insert(0, changelog());
        extra.insert(0, control());
        extra
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![file("foo.swo")]),
            tree(vec![file(".swp")]),
            tree(vec![dir("foo~")]),
            tree(vec![symlink("foo~")]),
        ];
        let fail = vec![
            tree(vec![file("foo~")]),
            tree(vec![file("debian/foo~")]),
            tree(vec![file("dir/a.swp")]),
            tree(vec![hardlink("notes~")]),
        ];
        SourceTreeTester::new(SourceContainsEditorBackupFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file("foo.swo")]),
            native(vec![changelog(), dir("foo~")]),
        ];
        let fail = vec![
            native(vec![changelog(), file("foo~")]),
            native(vec![changelog(), file(".pc/foo~")]),
            native(vec![changelog(), file("debian/foo~")]),
        ];
        SourcePackageTester::new(SourceContainsEditorBackupFile, pass, fail).test_and_snapshot();
    }
}
