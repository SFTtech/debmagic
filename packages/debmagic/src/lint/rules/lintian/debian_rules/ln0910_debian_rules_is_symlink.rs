use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct DebianRulesIsSymlink;

declare_rule! {
    /// `debian/rules` is a symbolic link instead of a regular file.
    ///
    /// Reported when the packaging File tree has `debian/rules` as a symlink,
    /// whatever it points at, including a dangling one. The link itself is
    /// judged, not its target. Extra is empty. Location is `debian/rules` with
    /// no line. Applies to the Source tree and Source package. Nothing is
    /// reported when `debian/rules` is missing.
    DebianRulesIsSymlink,
    code = "LN0910",
    tag = "debian-rules-is-symlink",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianRulesIsSymlink {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let is_symlink = ctx.files().is_some_and(|files| {
            files
                .get("debian/rules")
                .is_some_and(|entry| entry.kind() == EntryKind::Symlink)
        });
        if is_symlink {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("debian/rules"),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            vec![("debian/rules", TestFile::new("#!/usr/bin/make -f\n"))],
            // Only the file itself is judged, so a link inside `debian/` is fine.
            vec![
                ("debian/rules", TestFile::new("#!/usr/bin/make -f\n")),
                ("debian/other", TestFile::symlink("rules")),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/rules", TestFile::symlink("other")),
                ("debian/other", TestFile::new("#!/usr/bin/make -f\n")),
            ],
            // A dangling link is still a link.
            vec![("debian/rules", TestFile::symlink("missing"))],
        ];
        SourceTreeTester::new(DebianRulesIsSymlink, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/rules",
            TestFile::new("#!/usr/bin/make -f\n"),
        )])];
        let fail = vec![SourcePackageCase::from(vec![(
            "debian/rules",
            TestFile::symlink("other"),
        )])];
        SourcePackageTester::new(DebianRulesIsSymlink, pass, fail).test_and_snapshot();
    }
}
