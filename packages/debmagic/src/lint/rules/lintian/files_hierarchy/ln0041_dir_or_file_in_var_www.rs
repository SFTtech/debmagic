use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{flag_matching, non_whitespace_after};

pub struct DirOrFileInVarWww;

declare_rule! {
    /// The package ships something the Filesystem Hierarchy Standard forbids here.
    ///
    /// Flags paths with `var/www/` followed by a non-space character. Lintian's `files/hierarchy/standard` Check is
    /// one chain of branches, so a path an earlier branch handles is not
    /// flagged here (see `hierarchy_standard.rs`). Extra is empty; the Data
    /// files path (with a trailing slash for directories) is the Location.
    DirOrFileInVarWww,
    code = "LN0041",
    tag = "dir-or-file-in-var-www",
    default_selected = true,
    experimental = false,
}

pub(super) static MARK: u8 = 4;

pub(super) fn matches(name: &str, kind: EntryKind, udeb: bool, _package: &str) -> bool {
    let _ = (kind, udeb);
    non_whitespace_after(name, "var/www/")
}

impl BinaryPackageRule for DirOrFileInVarWww {
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
            tree(&[("var", true)]),
            tree(&[("other/var/www/x", false)]),
        ];
        let fail = vec![
            tree(&[("var/www/x", false)]),
            tree(&[("var/www/x", true), ("var/www/x/file", false)]),
        ];
        BinaryPackageTester::new(DirOrFileInVarWww, pass, fail).test_and_snapshot();
    }
}
