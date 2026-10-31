use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{direct_child, flag_matching};

pub struct FsstndDirInUsr;

declare_rule! {
    /// A direct `usr/` directory still uses an FSSTND name.
    ///
    /// Flags `usr/{dict,doc,etc,info,man,adm,preserve}/` on a Binary package
    /// that is not a udeb. Follows lintian's `files/hierarchy/standard` branch
    /// order (see `hierarchy_standard.rs`). Extra is empty; the directory, with
    /// a trailing slash, is the Location.
    FsstndDirInUsr,
    code = "LN0058",
    tag = "FSSTND-dir-in-usr",
    default_selected = true,
    experimental = false,
}

pub(super) static MARK: () = ();

pub(super) fn matches(name: &str, kind: EntryKind, udeb: bool, _package: &str) -> bool {
    let _ = kind;
    !udeb
        && direct_child(name, "usr/").is_some_and(|child| {
            matches!(
                child,
                "dict" | "doc" | "etc" | "info" | "man" | "adm" | "preserve"
            )
        })
}

impl BinaryPackageRule for FsstndDirInUsr {
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
            tree(&[("usr/bin", true)]),
            tree(&[("usr/share", true)]),
            tree(&[("usr/doc/changelog", false)]),
            tree(&[("usr/dictionary", true)]),
            tree(&[("usr/doc", true)]).udeb(),
        ];
        let fail = vec![
            tree(&[("usr/dict", true)]),
            tree(&[("usr/doc", true)]),
            tree(&[("usr/etc", true)]),
            tree(&[("usr/info", true)]),
            tree(&[("usr/man", true)]),
            tree(&[("usr/adm", true)]),
            tree(&[("usr/preserve", true)]),
            tree(&[("usr/doc", true), ("usr/doc/changelog", false)]),
        ];
        BinaryPackageTester::new(FsstndDirInUsr, pass, fail).test_and_snapshot();
    }
}
