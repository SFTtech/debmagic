use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{flag_matching, has_more};

pub struct DirOrFileInEtcOpt;

declare_rule! {
    /// The package ships something the Filesystem Hierarchy Standard forbids here.
    ///
    /// Flags paths with `etc/opt/` followed by at least one more character. Lintian's `files/hierarchy/standard` Check is
    /// one chain of branches, so a path an earlier branch handles is not
    /// flagged here (see `hierarchy_standard.rs`). Extra is empty; the Data
    /// files path (with a trailing slash for directories) is the Location.
    DirOrFileInEtcOpt,
    code = "LN0045",
    tag = "dir-or-file-in-etc-opt",
    default_selected = true,
    experimental = false,
}

pub(super) static MARK: u8 = 8;

pub(super) fn matches(name: &str, kind: EntryKind, udeb: bool, _package: &str) -> bool {
    let _ = (kind, udeb);
    has_more(name, "etc/opt/")
}

impl BinaryPackageRule for DirOrFileInEtcOpt {
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
            tree(&[("etc", true)]),
            tree(&[("other/etc/opt/x", false)]),
        ];
        let fail = vec![
            tree(&[("etc/opt/x", false)]),
            tree(&[("etc/opt/x", true), ("etc/opt/x/file", false)]),
        ];
        BinaryPackageTester::new(DirOrFileInEtcOpt, pass, fail).test_and_snapshot();
    }
}
