use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::flag::flag_prefix;

pub struct DirOrFileInSrv;

declare_rule! {
    /// The package ships something under `/srv`.
    ///
    /// Flags every file, directory and symlink whose path starts with
    /// `srv/` followed by at least one more character; `/srv` itself is
    /// not flagged. Udebs are checked too. Extra is empty; the Data files
    /// path (with a trailing slash for directories) is the Location.
    DirOrFileInSrv,
    code = "LN0035",
    tag = "dir-or-file-in-srv",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DirOrFileInSrv {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_prefix(ctx, "srv/");
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
            tree(&[("usr/share/srv", false), ("usr/srv/x", false)]),
            tree(&[("srv", true)]),
            tree(&[("srvx/file", false)]),
            tree(&[("var/srv/file", false)]),
        ];
        let fail = vec![
            tree(&[("srv/foo", true), ("srv/foo/bar", false)]),
            tree(&[("srv", true), ("srv/file", false)]),
            tree(&[("srv/file", false)]).udeb(),
        ];
        BinaryPackageTester::new(DirOrFileInSrv, pass, fail).test_and_snapshot();
    }
}
