use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::symlink_links::{SymlinkIssue, symlinks_with_issue};

pub struct SymlinkIsSelfRecursive;

declare_rule! {
    /// A symlink points at itself or at a directory above it.
    ///
    /// Fires for the target `/` and for relative targets that have nothing
    /// left after the leading `.` and `..` segments (`.`, `..`, `../..`).
    /// Extra is the link text as written; the Data files path is the Location.
    /// Udebs are checked too.
    SymlinkIsSelfRecursive,
    code = "LN0030",
    tag = "symlink-is-self-recursive",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SymlinkIsSelfRecursive {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for (path, link) in symlinks_with_issue(ctx, SymlinkIssue::SelfRecursive) {
            ctx.diagnostic(Diagnostic::warn(link).with_location(Location {
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
            links(&[("usr/lib/x/link", "../y")]),
            links(&[("usr/lib/x/link", "/usr")]),
            // Too many up segments: lintian stops judging the link.
            links(&[("link", "..")]),
        ];
        let fail = vec![
            links(&[("usr/lib/x/link", "/")]),
            links(&[("usr/lib/x/link", "//")]),
            links(&[("usr/lib/x/link", ".")]),
            links(&[("usr/lib/x/link", "..")]),
            links(&[("usr/lib/x/link", "../..")]),
            links(&[("usr/lib/x/link", "./.")]),
        ];
        BinaryPackageTester::new(SymlinkIsSelfRecursive, pass, fail).test_and_snapshot();
    }
}
