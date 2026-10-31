use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{flag_matching, non_whitespace_after};

pub struct DirInUsrLocal;

declare_rule! {
    /// The package ships something the Filesystem Hierarchy Standard forbids here.
    ///
    /// Flags paths with `usr/local/` followed by a non-space character, for directories. Lintian's `files/hierarchy/standard` Check is
    /// one chain of branches, so a path an earlier branch handles is not
    /// flagged here (see `hierarchy_standard.rs`). Extra is empty; the Data
    /// files path (with a trailing slash for directories) is the Location.
    DirInUsrLocal,
    code = "LN0046",
    tag = "dir-in-usr-local",
    default_selected = true,
    experimental = false,
}

pub(super) static MARK: () = ();

pub(super) fn matches(name: &str, kind: EntryKind, udeb: bool, _package: &str) -> bool {
    let _ = udeb;
    kind == EntryKind::Directory && non_whitespace_after(name, "usr/local/")
}

impl BinaryPackageRule for DirInUsrLocal {
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
            tree(&[("usr/local/x", false)]),
            tree(&[("usr/local", true)]),
        ];
        let fail = vec![
            tree(&[("usr/local/x", true)]),
            tree(&[("usr/local/x", true), ("usr/local/x/file", false)]),
        ];
        BinaryPackageTester::new(DirInUsrLocal, pass, fail).test_and_snapshot();
    }
}
