use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::hierarchy_standard::{direct_child, flag_matching_with};

pub struct NonStandardDirInUsr;

declare_rule! {
    /// A direct `usr/` directory is not in the FHS.
    ///
    /// On a Binary package that is not a udeb, a direct subdirectory of `usr/`
    /// is flagged unless it is an FHS name, `libexec`, or `lib64` / `lib32` /
    /// `libx32` (those last three belong to `non-multi-arch-lib-dir`). An
    /// earlier `files/hierarchy/standard` branch, including `FSSTND-dir-in-usr`,
    /// hides this one (see `hierarchy_standard.rs`). Extra is empty; the
    /// directory, with a trailing slash, is the Location.
    NonStandardDirInUsr,
    code = "LN0059",
    tag = "non-standard-dir-in-usr",
    default_selected = true,
    experimental = false,
}

pub(super) static MARK: u8 = 14;

/// Every direct `usr/` subdirectory on a non-udeb ends the chain. FHS names,
/// `libexec`, and the multi-arch lib directories are not flagged.
pub(super) fn claims(name: &str, _kind: EntryKind, udeb: bool, _package: &str) -> bool {
    !udeb && direct_child(name, "usr/").is_some()
}

pub(super) fn matches(name: &str, kind: EntryKind, udeb: bool, package: &str) -> bool {
    claims(name, kind, udeb, package)
        && direct_child(name, "usr/").is_some_and(|child| {
            !matches!(
                child,
                "X11R6"
                    | "X386"
                    | "bin"
                    | "games"
                    | "include"
                    | "lib"
                    | "local"
                    | "sbin"
                    | "share"
                    | "src"
                    | "spool"
                    | "tmp"
                    | "libexec"
                    | "lib64"
                    | "lib32"
                    | "libx32"
            )
        })
}

impl BinaryPackageRule for NonStandardDirInUsr {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_matching_with(ctx, &MARK, Severity::Warning);
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
            tree(&[("usr/lib", true)]),
            tree(&[("usr/libexec", true)]),
            tree(&[("usr/lib64", true)]),
            tree(&[("usr/lib32", true)]),
            tree(&[("usr/libx32", true)]),
            tree(&[("usr/share", true)]),
            tree(&[("usr/doc", true)]),
            tree(&[("usr/foo/bar", false)]),
            tree(&[("usr/foo", true)]).udeb(),
        ];
        let fail = vec![
            tree(&[("usr/foo", true)]),
            tree(&[("usr/libfoo", true)]),
            tree(&[("usr/foo", true), ("usr/foo/bar", false)]),
        ];
        BinaryPackageTester::new(NonStandardDirInUsr, pass, fail).test_and_snapshot();
    }
}
