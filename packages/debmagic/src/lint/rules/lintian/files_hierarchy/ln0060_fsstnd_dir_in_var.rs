use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{direct_child, flag_matching};

pub struct FsstndDirInVar;

declare_rule! {
    /// A direct `var/` directory still uses an FSSTND name.
    ///
    /// Flags `var/{adm,catman,named,nis,preserve}/` on a Binary package that is
    /// not a udeb. Follows lintian's `files/hierarchy/standard` branch order
    /// (see `hierarchy_standard.rs`). Extra is empty; the directory, with a
    /// trailing slash, is the Location.
    FsstndDirInVar,
    code = "LN0060",
    tag = "FSSTND-dir-in-var",
    default_selected = true,
    experimental = false,
}

pub(super) static MARK: () = ();

pub(super) fn matches(name: &str, kind: EntryKind, udeb: bool, _package: &str) -> bool {
    let _ = kind;
    !udeb
        && direct_child(name, "var/")
            .is_some_and(|child| matches!(child, "adm" | "catman" | "named" | "nis" | "preserve"))
}

impl BinaryPackageRule for FsstndDirInVar {
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
            tree(&[("var/lib", true)]),
            tree(&[("var/log", true)]),
            tree(&[("var/foo", true)]),
            tree(&[("var/adm/log", false)]),
            tree(&[("var/admin", true)]),
            tree(&[("var/adm", true)]).udeb(),
        ];
        let fail = vec![
            tree(&[("var/adm", true)]),
            tree(&[("var/catman", true)]),
            tree(&[("var/named", true)]),
            tree(&[("var/nis", true)]),
            tree(&[("var/preserve", true)]),
            tree(&[("var/adm", true), ("var/adm/log", false)]),
        ];
        BinaryPackageTester::new(FsstndDirInVar, pass, fail).test_and_snapshot();
    }
}
