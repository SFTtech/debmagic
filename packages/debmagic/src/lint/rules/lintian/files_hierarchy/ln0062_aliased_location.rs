use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{BranchId, flag_matching};

pub struct AliasedLocation;

declare_rule! {
    /// The package installs into an aliased top-level location.
    ///
    /// Flags anything under `bin/`, `sbin/`, or `lib` plus zero or more
    /// lowercase letters and digits (`lib64/`, `library`, …), including those
    /// directories themselves. Package `base-files` is exempt. Follows
    /// lintian's `files/hierarchy/standard` branch order (see
    /// `hierarchy_standard.rs`). Extra is empty; the Data files path (with a
    /// trailing slash for directories) is the Location. Udebs are checked too.
    AliasedLocation,
    code = "LN0062",
    tag = "aliased-location",
    default_selected = true,
    experimental = false,
}

pub(super) fn matches(name: &str, kind: EntryKind, udeb: bool, package: &str) -> bool {
    let _ = (kind, udeb);
    package != "base-files" && aliased(name)
}

fn aliased(name: &str) -> bool {
    let rest = if let Some(rest) = name.strip_prefix("sbin") {
        rest
    } else if let Some(rest) = name.strip_prefix("bin") {
        rest
    } else if let Some(rest) = name.strip_prefix("lib") {
        rest.trim_start_matches(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
    } else {
        return false;
    };
    rest.is_empty() || rest.starts_with('/')
}

impl BinaryPackageRule for AliasedLocation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_matching(ctx, BranchId::AliasedLocation);
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
            tree(&[("usr/bin/tool", false)]),
            tree(&[("usr/lib/libfoo", false)]),
            tree(&[("binary", false)]),
            tree(&[("lib_foo", false)]),
            tree(&[("sbin", true)]).package("base-files"),
            tree(&[("bin/tool", false)]).package("base-files"),
        ];
        let fail = vec![
            tree(&[("bin", true)]),
            tree(&[("bin/tool", false)]),
            tree(&[("sbin", true), ("sbin/tool", false)]),
            tree(&[("lib", true), ("lib/libfoo", false)]),
            tree(&[("lib64/libfoo", false)]),
            tree(&[("library", false)]),
            tree(&[("bin/tool", false)]).udeb(),
        ];
        BinaryPackageTester::new(AliasedLocation, pass, fail).test_and_snapshot();
    }
}
