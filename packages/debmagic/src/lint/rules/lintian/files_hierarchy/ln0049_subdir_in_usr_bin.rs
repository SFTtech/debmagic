use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{BranchId, flag_matching, has_more};

pub struct SubdirInUsrBin;

declare_rule! {
    /// A directory sits in `usr/bin`.
    ///
    /// Follows lintian's `files/hierarchy/standard` branch order (see
    /// `hierarchy_standard.rs`). Extra is empty; the Data files path (with a
    /// trailing slash for directories) is the Location. Udebs are checked too.
    SubdirInUsrBin,
    code = "LN0049",
    tag = "subdir-in-usr-bin",
    default_selected = true,
    experimental = false,
}

/// Anything under `usr/bin/` ends the chain. A directory other than `X11/`
/// and `mh/` is flagged.
pub(super) fn claims(name: &str, _kind: EntryKind, _udeb: bool, _package: &str) -> bool {
    name.starts_with("usr/bin/")
}

pub(super) fn matches(name: &str, kind: EntryKind, _udeb: bool, _package: &str) -> bool {
    kind == EntryKind::Directory
        && has_more(name, "usr/bin/")
        && !name.starts_with("usr/bin/X11/")
        && !name.starts_with("usr/bin/mh/")
}

impl BinaryPackageRule for SubdirInUsrBin {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_matching(ctx, BranchId::SubdirInUsrBin);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(entries: &[(&str, TestFile)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let file = || TestFile::new("x");
        let dir = TestFile::directory;
        let pass = vec![
            tree(&[]),
            tree(&[("usr/bin", dir()), ("usr/bin/tool", file())]),
            tree(&[("usr/bin/X11", dir()), ("usr/bin/mh", dir())]),
            tree(&[("usr/bin/X11/sub", dir())]),
            tree(&[("usr/bin/tool", TestFile::symlink("x"))]),
            tree(&[("usr/sbin/sub", dir())]),
        ];
        let fail = vec![
            tree(&[("usr/bin/foo", dir())]),
            tree(&[("usr/bin/bin", dir()), ("usr/bin/bin/x", file())]),
            tree(&[("usr/bin/mhx", dir())]),
            tree(&[("usr/bin/foo", dir())]).udeb(),
        ];
        BinaryPackageTester::new(SubdirInUsrBin, pass, fail).test_and_snapshot();
    }
}
