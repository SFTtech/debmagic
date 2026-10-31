use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::symlink_links::{SymlinkIssue, emit_issue};
use crate::lint::rule::Severity;

pub struct CompressedSymlinkWithWrongExt;

declare_rule! {
    /// A symlink points at a compressed file but its own name lacks that extension.
    ///
    /// The link text, as written, ends in `.bz`, `.bz2`, `.gz`, `.tgz`, `.xz`,
    /// `.z`, or `.Z`, with optional trailing whitespace. The symlink path must
    /// end in the same extension. Udebs are checked too. Extra is the link
    /// text; the Data files path is the Location.
    CompressedSymlinkWithWrongExt,
    code = "LN0075",
    tag = "compressed-symlink-with-wrong-ext",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CompressedSymlinkWithWrongExt {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        emit_issue(ctx, SymlinkIssue::CompressedWrongExt, Severity::Error);
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
            links(&[("usr/share/doc/pkg/changelog.gz", "changelog.gz")]),
            links(&[(
                "usr/share/doc/libbaz2/changelog.gz",
                "/usr/share/doc/lintian/changelog.gz",
            )]),
            links(&[("usr/bin/tool", "tool")]),
        ];
        let fail = vec![
            links(&[(
                "usr/lib/lintian/data/comp-data",
                "../../../share/lintian/data/comp-data.gz",
            )]),
            links(&[("usr/share/doc/pkg/README", "README.bz2")]),
            links(&[("usr/share/doc/pkg/README", "README.gz ")]).udeb(),
        ];
        BinaryPackageTester::new(CompressedSymlinkWithWrongExt, pass, fail).test_and_snapshot();
    }
}
