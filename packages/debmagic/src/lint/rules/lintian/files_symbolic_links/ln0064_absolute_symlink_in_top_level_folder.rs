use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::links::{SymlinkIssue, emit_issue};

pub struct AbsoluteSymlinkInTopLevelFolder;

declare_rule! {
    /// An absolute symlink stays inside one top-level directory.
    ///
    /// After repeated slashes are collapsed and one trailing slash is dropped,
    /// the target starts with `/` and its first component equals the symlink's
    /// first component. Udebs are not checked. Extra is the link text as
    /// written; the Data files path is the Location.
    AbsoluteSymlinkInTopLevelFolder,
    code = "LN0064",
    tag = "absolute-symlink-in-top-level-folder",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for AbsoluteSymlinkInTopLevelFolder {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        emit_issue(ctx, SymlinkIssue::AbsoluteInTopLevel, Severity::Warning);
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
            links(&[("usr/bin/tool", "/var/lib/tool")]),
            links(&[("usr/bin/tool", "../lib/tool")]),
            links(&[("usr/share/lintian/data/self-recursive", "/")]),
            links(&[(
                "usr/share/lintian/data/data-file.old",
                "/usr/share/lintian/data/data-file",
            )])
            .udeb(),
        ];
        let fail = vec![
            links(&[(
                "usr/share/lintian/data/data-file.old",
                "/usr/share/lintian/data/data-file",
            )]),
            links(&[(
                "usr/lib/filenames/symlink3wrong",
                "/usr/lib/filenames/symlink2",
            )]),
            links(&[(
                "usr/share/doc/binary/html/ch3.html",
                "/usr/share/doc/binary/htm/ch1.html",
            )]),
        ];
        BinaryPackageTester::new(AbsoluteSymlinkInTopLevelFolder, pass, fail).test_and_snapshot();
    }
}
