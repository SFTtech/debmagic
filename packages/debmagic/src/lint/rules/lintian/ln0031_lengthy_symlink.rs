use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::symlink_links::{SymlinkIssue, symlinks_with_issue};

pub struct LengthySymlink;

declare_rule! {
    /// A relative symlink goes up out of a directory and back into it.
    ///
    /// After the leading `..` segments, the first remaining segment equals
    /// the directory the last `..` left (`usr/share/doc/bar/star` ->
    /// `../bar/foo`), so the link could be shorter. Extra is the link text as
    /// written; the Data files path is the Location. Udebs are checked too.
    LengthySymlink,
    code = "LN0031",
    tag = "lengthy-symlink",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for LengthySymlink {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for (path, link) in symlinks_with_issue(ctx, SymlinkIssue::Lengthy) {
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
            links(&[("usr/share/doc/bar/star", "../baz/foo")]),
            links(&[("usr/share/doc/bar/star", "foo")]),
            links(&[("usr/share/doc/bar/star", "/bar/foo")]),
        ];
        let fail = vec![
            links(&[("usr/share/doc/bar/star", "../bar/foo")]),
            links(&[("usr/lib/filenames/symlink4wrong", "../filenames/symlink2")]),
            links(&[("usr/share/doc/binary/html/ch2.html", "../html/./ch1.html")]),
            links(&[("usr/share/doc/bar/star", "../../doc/bar/foo")]),
        ];
        BinaryPackageTester::new(LengthySymlink, pass, fail).test_and_snapshot();
    }
}
