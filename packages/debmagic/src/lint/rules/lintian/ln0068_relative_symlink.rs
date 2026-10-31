use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::symlink_links::{SymlinkIssue, emit_issue};
use crate::lint::rule::Severity;

pub struct RelativeSymlink;

declare_rule! {
    /// A relative symlink leaves its top-level directory.
    ///
    /// Judged after repeated slashes are collapsed and one trailing slash is
    /// dropped, and only once the leading `..` segments have reached the
    /// root. Too many `..` segments stop the judgement before this Tag.
    /// Udebs still match when the next component differs from the symlink's
    /// top-level directory. Extra is the link text as written; the Data
    /// files path is the Location.
    RelativeSymlink,
    code = "LN0068",
    tag = "relative-symlink",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for RelativeSymlink {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        emit_issue(ctx, SymlinkIssue::Relative, Severity::Error);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn links(links: &[(&str, &str)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            links
                .iter()
                .map(|(name, target)| (name.to_string(), TestFile::symlink(*target)))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            links(&[]),
            links(&[("usr/bin/tool", "../lib/tool")]),
            links(&[("usr/bin/tool", "../../usr/bin/other")]),
            // Too many up segments: lintian stops before this Tag.
            links(&[("usr/lib/filenames/symlink2wrong", "../../../../etc/symlink")]),
            links(&[("usr/bin/tool", "/etc/tool")]),
        ];
        let fail = vec![
            links(&[(
                "usr/share/lintian/data/pkg-old.conf",
                "../../../../etc/lintian/pkg.conf",
            )]),
            links(&[("usr/lib/filenames/symlink1wrong", "../../../etc/symlink")]),
            links(&[("usr/lib/filenames/symlink10wrong", "../../..")]),
            links(&[("usr/lib/filenames/symlink1wrong", "../../../etc/symlink")]).udeb(),
        ];
        BinaryPackageTester::new(RelativeSymlink, pass, fail).test_and_snapshot();
    }
}
