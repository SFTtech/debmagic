use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{BranchId, direct_child, flag_matching, has_more};

pub struct NonStandardDirInVar;

declare_rule! {
    /// A direct `var/` directory is not in the FHS, or something sits under `var/lib/games`.
    ///
    /// On a Binary package that is not a udeb, a direct subdirectory of `var/`
    /// is flagged unless it is an FHS name. Package `base-files` may ship
    /// `var/backups/` and `var/local/`. Anything under `var/lib/games/` is
    /// flagged too. An earlier `files/hierarchy/standard` branch, including
    /// `FSSTND-dir-in-var`, hides this one (see `hierarchy_standard.rs`). Extra
    /// is empty; the Data files path (with a trailing slash for directories)
    /// is the Location.
    NonStandardDirInVar,
    code = "LN0061",
    tag = "non-standard-dir-in-var",
    default_selected = true,
    experimental = false,
}

/// Every direct `var/` subdirectory on a non-udeb ends the chain. FHS names,
/// and `base-files`'s `backups` and `local`, are not flagged.
pub(super) fn claims(name: &str, _kind: EntryKind, udeb: bool, _package: &str) -> bool {
    !udeb && direct_child(name, "var/").is_some()
}

pub(super) fn matches(name: &str, kind: EntryKind, udeb: bool, package: &str) -> bool {
    let _ = kind;
    let Some(child) = direct_child(name, "var/") else {
        return false;
    };
    if udeb || (package == "base-files" && matches!(child, "backups" | "local")) {
        return false;
    }
    !matches!(
        child,
        "account"
            | "lib"
            | "cache"
            | "crash"
            | "games"
            | "lock"
            | "log"
            | "opt"
            | "run"
            | "spool"
            | "state"
            | "tmp"
            | "www"
            | "yp"
    )
}

/// `var/lib/games/` with something after it. The directory itself is not flagged.
pub(super) fn matches_games(name: &str, kind: EntryKind, udeb: bool, _package: &str) -> bool {
    let _ = kind;
    !udeb && has_more(name, "var/lib/games/")
}

impl BinaryPackageRule for NonStandardDirInVar {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_matching(ctx, BranchId::NonStandardDirInVar);
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
            tree(&[("var/cache", true)]),
            tree(&[("var/lock", true)]),
            tree(&[("var/www", true)]),
            tree(&[("var/adm", true)]),
            tree(&[("var/lib/games", true)]),
            tree(&[("var/foo", true)]).udeb(),
            tree(&[("var/lib/games/score", false)]).udeb(),
            tree(&[("var/backups", true)]).package("base-files"),
            tree(&[("var/local", true)]).package("base-files"),
        ];
        let fail = vec![
            tree(&[("var/foo", true)]),
            tree(&[("var/backups", true)]),
            tree(&[("var/local", true)]),
            tree(&[("var/foo", true)]).package("base-files"),
            tree(&[("var/lib/games/score", false)]),
            tree(&[("var/lib/games/x", true)]),
        ];
        BinaryPackageTester::new(NonStandardDirInVar, pass, fail).test_and_snapshot();
    }
}
