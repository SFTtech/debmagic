use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::hierarchy_standard::{Hierarchy, hierarchy_paths};

pub struct FileDirectlyInUsrShare;

declare_rule! {
    /// A regular file sits directly in `usr/share`.
    ///
    /// Follows lintian's `files/hierarchy/standard` branch order (see
    /// `hierarchy_standard.rs`). Extra is empty; the Data files path (with a
    /// trailing slash for directories) is the Location. Udebs are checked too.
    FileDirectlyInUsrShare,
    code = "LN0048",
    tag = "file-directly-in-usr-share",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for FileDirectlyInUsrShare {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for path in hierarchy_paths(ctx, Hierarchy::FileDirectlyInUsrShare) {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
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
            tree(&[("usr/share", dir())]),
            tree(&[("usr/share/x", dir()), ("usr/share/x/file", file())]),
            tree(&[("usr/share/link", TestFile::symlink("x"))]),
            tree(&[("usr/share/doc/file", file())]),
            tree(&[("usr/shared", file())]),
        ];
        let fail = vec![
            tree(&[("usr/share/bar", file())]),
            tree(&[("usr/share/baz", file()), ("usr/share/qux", file())]),
            tree(&[("usr/share/bar", file())]).udeb(),
        ];
        BinaryPackageTester::new(FileDirectlyInUsrShare, pass, fail).test_and_snapshot();
    }
}
