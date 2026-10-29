use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::symlink_links::{SymlinkIssue, symlinks_with_issue};

pub struct SymlinkContainsSpuriousSegments;

declare_rule! {
    /// A symlink's target has `.` or `..` segments that serve no purpose.
    ///
    /// Judged on the link text after repeated slashes are collapsed and one
    /// trailing slash is dropped, for relative targets only. A `.` segment
    /// among the leading `..` segments counts each time (a target of exactly
    /// `.` does not); after the leading `..` segments, one hit is reported if
    /// any later segment is `.` or `..`. Extra is the link text as written;
    /// the Data files path is the Location. Udebs are checked too.
    SymlinkContainsSpuriousSegments,
    code = "LN0029",
    tag = "symlink-contains-spurious-segments",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SymlinkContainsSpuriousSegments {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for (path, link) in symlinks_with_issue(ctx, SymlinkIssue::SpuriousSegments) {
            ctx.diagnostic(Diagnostic::error(link).with_location(Location {
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
            links(&[
                ("usr/bin/tool", "../lib/tool/tool"),
                ("usr/bin/other", "/usr/lib/other"),
            ]),
            BinaryPackageCase::from(vec![("usr/bin/file", TestFile::new("x"))]),
            links(&[("usr/lib/x/link", "../y/z")]),
            links(&[("usr/bin/tool", "tool-1.0")]),
            // Too many up segments: lintian stops judging the link.
            links(&[("link", "../x/./y")]),
            // Absolute targets are not judged here.
            links(&[("usr/lib/x/link", "/usr/./lib")]),
        ];
        let fail = vec![
            links(&[("usr/lib/x/link", "./file")]),
            links(&[("usr/lib/x/link", "../menu/./something")]),
            links(&[("usr/lib/x/link", "../menu/../other")]),
            links(&[("usr/lib/x/link", "././file")]),
            links(&[("usr/lib/x/link", "../x/./file")]),
            links(&[("usr/lib/x/link", "./a/")]),
        ];
        BinaryPackageTester::new(SymlinkContainsSpuriousSegments, pass, fail).test_and_snapshot();
    }
}
