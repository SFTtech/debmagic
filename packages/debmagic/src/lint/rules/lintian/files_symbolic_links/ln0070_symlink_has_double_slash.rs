use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::links::{SymlinkIssue, emit_issue};

pub struct SymlinkHasDoubleSlash;

declare_rule! {
    /// A symlink target contains `//`.
    ///
    /// Judged on the link text as written, before slashes are collapsed.
    /// Udebs are checked too. Extra is that link text; the Data files path
    /// is the Location.
    SymlinkHasDoubleSlash,
    code = "LN0070",
    tag = "symlink-has-double-slash",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SymlinkHasDoubleSlash {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        emit_issue(ctx, SymlinkIssue::DoubleSlash, Severity::Warning);
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
        let pass = vec![links(&[]), links(&[("usr/bin/tool", "/usr/bin/tool")])];
        let fail = vec![
            links(&[(
                "usr/share/lintian/data/pkg.conf",
                "../..//..//..//../etc/lintian/pkg.conf",
            )]),
            links(&[("usr/lib/filenames/symlink8wrong", "../menu//something")]),
            links(&[("usr/bin/tool", "//usr/bin/tool")]).udeb(),
        ];
        BinaryPackageTester::new(SymlinkHasDoubleSlash, pass, fail).test_and_snapshot();
    }
}
