use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{flag_matching, has_more};

pub struct DirOrFileInRun;

declare_rule! {
    /// The package ships something the Filesystem Hierarchy Standard forbids here.
    ///
    /// Flags paths with `run/` followed by at least one more character (not for udebs). Lintian's `files/hierarchy/standard` Check is
    /// one chain of branches, so a path an earlier branch handles is not
    /// flagged here (see `hierarchy_standard.rs`). Extra is empty; the Data
    /// files path (with a trailing slash for directories) is the Location.
    DirOrFileInRun,
    code = "LN0044",
    tag = "dir-or-file-in-run",
    default_selected = true,
    experimental = false,
}

pub(super) static MARK: () = ();

pub(super) fn matches(name: &str, kind: EntryKind, udeb: bool, _package: &str) -> bool {
    let _ = kind;
    !udeb && has_more(name, "run/")
}

impl BinaryPackageRule for DirOrFileInRun {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_matching(ctx, &MARK);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::BinaryPackageTester;

    use super::super::tree::tree;
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&[("run", true)]),
            tree(&[("other/run/x", false)]),
            tree(&[("run/x", false)]).udeb(),
        ];
        let fail = vec![
            tree(&[("run/x", false)]),
            tree(&[("run/x", true), ("run/x/file", false)]),
        ];
        BinaryPackageTester::new(DirOrFileInRun, pass, fail).test_and_snapshot();
    }
}
