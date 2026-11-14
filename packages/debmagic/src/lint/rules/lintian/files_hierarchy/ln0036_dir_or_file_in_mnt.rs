use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::flag::flag_prefix;

pub struct DirOrFileInMnt;

declare_rule! {
    /// The package ships something under `/mnt`.
    ///
    /// Flags every file, directory and symlink whose path starts with
    /// `mnt/` followed by at least one more character; `/mnt` itself is
    /// not flagged. Udebs are checked too. Extra is empty; the Data files
    /// path (with a trailing slash for directories) is the Location.
    DirOrFileInMnt,
    code = "LN0036",
    tag = "dir-or-file-in-mnt",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DirOrFileInMnt {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_prefix(ctx, "mnt/");
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
            tree(&[("usr/share/mnt", false), ("usr/mnt/x", false)]),
            tree(&[("mnt", true)]),
            tree(&[("mntx/file", false)]),
            tree(&[("var/mnt/file", false)]),
        ];
        let fail = vec![
            tree(&[("mnt/foo", true), ("mnt/foo/bar", false)]),
            tree(&[("mnt", true), ("mnt/file", false)]),
            tree(&[("mnt/file", false)]).udeb(),
        ];
        BinaryPackageTester::new(DirOrFileInMnt, pass, fail).test_and_snapshot();
    }
}
