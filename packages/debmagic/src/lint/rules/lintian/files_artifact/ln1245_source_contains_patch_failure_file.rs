use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native};

pub struct SourceContainsPatchFailureFile;

declare_rule! {
    /// A native package contains a patch failure file.
    ///
    /// Only for a native package (`3.0 (native)`, or `1.0` without an orig
    /// tarball). Regular files and hardlinks whose path ends in `.orig` or
    /// `.rej`. Every patched path counts, including under `.pc/`. Extra is
    /// empty; the path is the Location. The non-native case is
    /// `debian-adds-patch-failure-file`.
    SourceContainsPatchFailureFile,
    code = "LN1245",
    tag = "source-contains-patch-failure-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for SourceContainsPatchFailureFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native(ctx, Artifact::PatchFailureFile);
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
            tree(vec![file("foo.rejected")]),
            tree(vec![file("foo.origin")]),
            tree(vec![dir("foo.rej")]),
            tree(vec![symlink("foo.rej")]),
        ];
        let fail = vec![
            tree(vec![file("foo.rej")]),
            tree(vec![file("debian/foo.orig")]),
            tree(vec![hardlink("notes.rej")]),
        ];
        SourceTreeTester::new(SourceContainsPatchFailureFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file("foo.rejected")]),
            native(vec![changelog(), dir("foo.rej")]),
        ];
        let fail = vec![
            native(vec![changelog(), file("foo.rej")]),
            native(vec![changelog(), file(".pc/foo.orig")]),
        ];
        SourcePackageTester::new(SourceContainsPatchFailureFile, pass, fail).test_and_snapshot();
    }
}
