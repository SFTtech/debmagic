use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::links::{SymlinkIssue, emit_issue};
use crate::lint::rule::Severity;

pub struct SymlinkHasTooManyUpSegments;

declare_rule! {
    /// A relative symlink has more leading `..` segments than its directory depth.
    ///
    /// Judged after repeated slashes are collapsed and one trailing slash is
    /// dropped. Lintian stops the rest of the relative-link judgement here.
    /// Udebs are checked too. Extra is the link text as written; the Data
    /// files path is the Location.
    SymlinkHasTooManyUpSegments,
    code = "LN0071",
    tag = "symlink-has-too-many-up-segments",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SymlinkHasTooManyUpSegments {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        emit_issue(ctx, SymlinkIssue::TooManyUpSegments, Severity::Error);
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
            links(&[("usr/lib/filenames/symlink10wrong", "../../..")]),
            links(&[("usr/bin/tool", "../lib/tool")]),
        ];
        let fail = vec![
            links(&[(
                "usr/share/lintian/data/pkg.conf",
                "../..//..//..//../etc/lintian/pkg.conf",
            )]),
            links(&[("usr/lib/filenames/symlink2wrong", "../../../../etc/symlink")]),
            links(&[("a/link", "../../x")]).udeb(),
        ];
        BinaryPackageTester::new(SymlinkHasTooManyUpSegments, pass, fail).test_and_snapshot();
    }
}
