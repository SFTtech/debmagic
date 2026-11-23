use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native};

pub struct SourceContainsQuiltControlDir;

declare_rule! {
    /// A native package contains a quilt control directory (`.pc`).
    ///
    /// A directory named `.pc`, at any depth; `debian/.pc` and `src/.pc/`
    /// count, a file or symlink named `.pc` does not, and nor does a `.pc`
    /// nested inside it. Only for a native package (`3.0 (native)`, or
    /// `1.0` without an orig tarball, per lintian's `native`), where no
    /// quilt unpack directory is dropped. Extra is empty; the directory
    /// path with a trailing slash is the Location. The non-native case is
    /// `debian-adds-quilt-control-dir`.
    SourceContainsQuiltControlDir,
    code = "LN1772",
    tag = "source-contains-quilt-control-dir",
    default_selected = true,
    experimental = false,
}

impl SourceRule for SourceContainsQuiltControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native(ctx, Artifact::QuiltControlDir);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::super::testing::{
        added, changelog, control, dir, file, native, native_tree, quilt_dsc, symlink, text,
    };
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            native_tree(vec![file(".pc")]),
            native_tree(vec![symlink(".pc")]),
            native_tree(vec![dir(".pc-like"), dir("pc")]),
            native_tree(vec![dir(".pc/applied"), file(".pc/applied/x")]),
            // Not native: a quilt format file.
            vec![
                control(),
                dir(".pc"),
                text("debian/source/format", "3.0 (quilt)\n"),
            ],
        ];
        let fail = vec![
            native_tree(vec![dir(".pc")]),
            native_tree(vec![dir("debian/.pc")]),
            native_tree(vec![dir("src/.pc"), dir("src/.pc/applied")]),
            native_tree(vec![dir(".pc/.pc")]),
            // Native by a `3.0 (native)` format file.
            vec![
                control(),
                dir(".pc"),
                text("debian/source/format", "3.0 (native)\n"),
            ],
        ];
        SourceTreeTester::new(SourceContainsQuiltControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), file(".pc")]),
            native(vec![changelog(), symlink(".pc")]),
            // Non-native: the Dsc names a quilt format, so the debian-adds
            // Tag applies instead.
            SourcePackageCase::from(vec![dir(".pc")]).dsc(quilt_dsc()),
            added(vec![dir(".pc")]),
        ];
        let fail = vec![
            native(vec![changelog(), dir(".pc")]),
            native(vec![changelog(), dir("debian/.pc")]),
            SourcePackageCase::from(vec![changelog(), dir(".pc")]).dsc(
                Dsc::parse("Format: 3.0 (native)\nVersion: 1.0\nSource: example\n").expect("dsc"),
            ),
            // A changelog with a revision, but no orig and no quilt format:
            // Version without a revision counts as native only when there is
            // no orig; here the Dsc names no `.diff.gz`, so native.
            SourcePackageCase::from(vec![
                (
                    "debian/changelog".to_string(),
                    TestFile::new(
                        "example (1.0) unstable; urgency=medium\n\n  * Initial.\n\n \
                         -- Jane <jane@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n",
                    ),
                ),
                dir(".pc"),
            ]),
        ];
        SourcePackageTester::new(SourceContainsQuiltControlDir, pass, fail).test_and_snapshot();
    }
}
