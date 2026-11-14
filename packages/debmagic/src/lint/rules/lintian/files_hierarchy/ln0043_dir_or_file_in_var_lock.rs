use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{BranchId, flag_matching, has_more};

pub struct DirOrFileInVarLock;

declare_rule! {
    /// The package ships something the Filesystem Hierarchy Standard forbids here.
    ///
    /// Flags paths with `var/lock/` followed by at least one more character (not for udebs). Lintian's `files/hierarchy/standard` Check is
    /// one chain of branches, so a path an earlier branch handles is not
    /// flagged here (see `hierarchy_standard.rs`). Extra is empty; the Data
    /// files path (with a trailing slash for directories) is the Location.
    DirOrFileInVarLock,
    code = "LN0043",
    tag = "dir-or-file-in-var-lock",
    default_selected = true,
    experimental = false,
}

pub(super) fn matches(name: &str, kind: EntryKind, udeb: bool, _package: &str) -> bool {
    let _ = kind;
    !udeb && has_more(name, "var/lock/")
}

impl BinaryPackageRule for DirOrFileInVarLock {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_matching(ctx, BranchId::DirOrFileInVarLock);
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
            tree(&[("var", true)]),
            tree(&[("other/var/lock/x", false)]),
            tree(&[("var/lock/x", false)]).udeb(),
        ];
        let fail = vec![
            tree(&[("var/lock/x", false)]),
            tree(&[("var/lock/x", true), ("var/lock/x/file", false)]),
        ];
        BinaryPackageTester::new(DirOrFileInVarLock, pass, fail).test_and_snapshot();
    }
}
