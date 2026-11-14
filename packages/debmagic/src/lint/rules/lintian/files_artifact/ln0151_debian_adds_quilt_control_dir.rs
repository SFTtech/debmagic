use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsQuiltControlDir;

declare_rule! {
    /// Debian added a quilt control directory (`.pc`).
    ///
    /// Directories only. On a quilt Source package, paths matching `^.pc/`
    /// are the unpack directory and are dropped before this pattern, so the
    /// root `.pc` does not fire. `debian/.pc` still does. A Source tree has
    /// no orig and does not drop `.pc`. Extra is empty; the directory path
    /// with a trailing slash is the Location.
    DebianAddsQuiltControlDir,
    code = "LN0151",
    tag = "debian-adds-quilt-control-dir",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsQuiltControlDir {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::QuiltControlDir);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageTester, SourceTreeTester};

    use super::super::testing::{
        added, control, dir, file, in_orig, native, quilt, quilt_format, symlink, text,
        v1_dsc_over_quilt_format,
    };
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            vec![control(), dir("pc")],
            vec![control(), file(".pc")],
            vec![control(), symlink(".pc")],
            vec![control(), dir(".pc/applied")],
        ];
        let fail = vec![
            vec![control(), dir(".pc")],
            vec![control(), dir("debian/.pc")],
            vec![control(), dir("src/.pc")],
            // Source tree: a quilt format file does not drop `.pc`.
            vec![
                control(),
                dir(".pc"),
                text("debian/source/format", "3.0 (quilt)\n"),
            ],
            vec![control(), dir(".pc"), dir("debian/.pc")],
        ];
        SourceTreeTester::new(DebianAddsQuiltControlDir, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![dir(".pc")]),
            in_orig(vec![dir(".pc")]),
            in_orig(vec![dir("debian/.pc")]),
            quilt(vec![dir(".pc")]),
            quilt(vec![dir(".pc"), dir(".pc/applied"), file(".pc/foo")]),
            quilt_format(vec![dir(".pc")]),
            added(vec![file(".pc")]),
            added(vec![symlink(".pc")]),
            added(vec![dir(".pc/applied")]),
        ];
        let fail = vec![
            added(vec![dir(".pc")]),
            added(vec![dir("debian/.pc")]),
            added(vec![dir("src/.pc")]),
            quilt(vec![dir("debian/.pc")]),
            quilt(vec![dir(".pc"), dir("debian/.pc")]),
            v1_dsc_over_quilt_format(vec![dir(".pc")]),
            added(vec![dir(".pc"), text("debian/source/format", "1.0\n")]),
        ];
        SourcePackageTester::new(DebianAddsQuiltControlDir, pass, fail).test_and_snapshot();
    }
}
