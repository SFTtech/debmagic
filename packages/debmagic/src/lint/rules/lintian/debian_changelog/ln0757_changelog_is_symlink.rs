use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct ChangelogIsSymlink;

declare_rule! {
    /// `debian/changelog` is a symbolic link.
    ///
    /// Checked on the Source tree or Source package Patched files, whether or
    /// not the changelog parses or has an entry. Any symlink at that path is
    /// flagged, including one whose target is missing. Extra is empty. The
    /// Location is `debian/changelog` with no line.
    ChangelogIsSymlink,
    code = "LN0757",
    tag = "changelog-is-symlink",
    default_selected = true,
    experimental = false,
}

impl SourceRule for ChangelogIsSymlink {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let is_symlink = files
            .get(Path::new("debian/changelog"))
            .is_some_and(|entry| entry.kind() == EntryKind::Symlink);
        if is_symlink {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("debian/changelog"),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const CHANGELOG: &str = "example (1.0-1) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            vec![("debian/changelog", TestFile::new(CHANGELOG))],
            // Other names and other paths are not the changelog.
            vec![("debian/changelog.gz", TestFile::symlink("changelog"))],
            vec![("changelog", TestFile::symlink("debian/changelog"))],
        ];
        let fail = vec![
            vec![("debian/changelog", TestFile::symlink("../changelog"))],
            vec![("debian/changelog", TestFile::symlink("changelog.real"))],
            vec![
                ("debian/changelog", TestFile::symlink("changelog.real")),
                ("debian/changelog.real", TestFile::new(CHANGELOG)),
            ],
        ];
        SourceTreeTester::new(ChangelogIsSymlink, pass, fail).test_and_snapshot();
    }
}
