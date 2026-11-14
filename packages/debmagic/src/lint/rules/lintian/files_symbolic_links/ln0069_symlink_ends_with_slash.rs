use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::links::{SymlinkIssue, emit_issue};

pub struct SymlinkEndsWithSlash;

declare_rule! {
    /// A symlink target ends with a slash after repeated slashes are collapsed.
    ///
    /// A target of `/` does not match: a character is required before the
    /// slash. Extra is the link text as written, not the collapsed form.
    /// Udebs are checked too. The Data files path is the Location.
    SymlinkEndsWithSlash,
    code = "LN0069",
    tag = "symlink-ends-with-slash",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SymlinkEndsWithSlash {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        emit_issue(ctx, SymlinkIssue::EndsWithSlash, Severity::Warning);
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
            links(&[("usr/bin/tool", "/etc/lintian"), ("usr/bin/root", "/")]),
        ];
        let fail = vec![
            links(&[("usr/share/etc-lintian", "/etc/lintian/")]),
            links(&[("usr/lib/filenames/symlink9wrong", "../menu/something/")]),
            links(&[("usr/bin/tool", "foo//")]).udeb(),
        ];
        BinaryPackageTester::new(SymlinkEndsWithSlash, pass, fail).test_and_snapshot();
    }
}
