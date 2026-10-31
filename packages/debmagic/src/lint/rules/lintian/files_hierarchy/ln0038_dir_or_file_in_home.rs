use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{flag_matching, has_more};

pub struct DirOrFileInHome;

declare_rule! {
    /// The package ships something the Filesystem Hierarchy Standard forbids here.
    ///
    /// Flags paths with `home/` or `root/` followed by at least one more character. Lintian's `files/hierarchy/standard` Check is
    /// one chain of branches, so a path an earlier branch handles is not
    /// flagged here (see `hierarchy_standard.rs`). Extra is empty; the Data
    /// files path (with a trailing slash for directories) is the Location.
    DirOrFileInHome,
    code = "LN0038",
    tag = "dir-or-file-in-home",
    default_selected = true,
    experimental = false,
}

pub(super) static MARK: u8 = 1;

pub(super) fn matches(name: &str, kind: EntryKind, udeb: bool, _package: &str) -> bool {
    let _ = (kind, udeb);
    has_more(name, "home/") || has_more(name, "root/")
}

impl BinaryPackageRule for DirOrFileInHome {
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
            tree(&[("home", true)]),
            tree(&[("other/home/user", false)]),
        ];
        let fail = vec![
            tree(&[("home/user", false)]),
            tree(&[("home/user", true), ("home/user/file", false)]),
            tree(&[("root/x", false)]),
        ];
        BinaryPackageTester::new(DirOrFileInHome, pass, fail).test_and_snapshot();
    }
}
