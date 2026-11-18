use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

/// A commented-out `Vcs-*` line dh_make writes, pointing at the literal
/// `<pkg>` placeholder under `collab-maint`. Anchored at the line start, not
/// at the end, as lintian's pattern is.
static DH_MAKE_VCS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^#\s*Vcs-(?:Git|Browser):\s*(?:git|http)://git\.debian\.org/(?:\?p=)?collab-maint/<pkg>\.git",
    )
    .expect("valid regex")
});

pub struct ControlFileContainsDhMakeVcsComment;

declare_rule! {
    /// `debian/control` still has the dh_make `Vcs-*` comment.
    ///
    /// The first line of `debian/control` that starts with `#`, then
    /// `Vcs-Git:` or `Vcs-Browser:`, then a `git://` or `http://` URL on
    /// `git.debian.org` under `collab-maint/<pkg>.git` (the literal `<pkg>`).
    /// Trailing whitespace is dropped. One Diagnostic at most. Extra is that
    /// line; the Location is `debian/control` with its line number.
    ControlFileContainsDhMakeVcsComment,
    code = "LN0846",
    tag = "control-file-contains-dh-make-vcs-comment",
    default_selected = true,
    experimental = false,
}

impl SourceRule for ControlFileContainsDhMakeVcsComment {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(entry) = files.get(Path::new("debian/control")) else {
            return;
        };
        if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            return;
        }
        let Ok(bytes) = files.read(entry) else {
            return;
        };
        let text = String::from_utf8_lossy(&bytes);
        let found: Option<(u32, String)> = text.split('\n').enumerate().find_map(|(index, raw)| {
            let line = raw.trim_end();
            DH_MAKE_VCS.is_match(line).then(|| {
                (
                    u32::try_from(index + 1).unwrap_or(u32::MAX),
                    line.to_string(),
                )
            })
        });
        if let Some((line, extra)) = found {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                path: PathBuf::from("debian/control"),
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const HEAD: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n";

    const TAIL: &str = "\nPackage: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn control(lines: &[&str]) -> Vec<(String, TestFile)> {
        let mut text = HEAD.to_string();
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        text.push_str(TAIL);
        vec![("debian/control".to_string(), TestFile::new(text))]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README".to_string(), TestFile::new("x\n"))],
            control(&[]),
            control(&["#Vcs-Git: https://salsa.debian.org/example.git"]),
            control(&["#Vcs-Git: git://git.debian.org/collab-maint/foo.git"]),
            control(&["Vcs-Git: git://git.debian.org/collab-maint/<pkg>.git"]),
            control(&[" #Vcs-Git: git://git.debian.org/collab-maint/<pkg>.git"]),
            control(&["#Vcs-Svn: git://git.debian.org/collab-maint/<pkg>.git"]),
        ];
        let fail = vec![
            control(&["#Vcs-Git: git://git.debian.org/collab-maint/<pkg>.git"]),
            control(&["# Vcs-Browser: http://git.debian.org/?p=collab-maint/<pkg>.git;a=summary"]),
            control(&["#Vcs-Git: git://git.debian.org/collab-maint/<pkg>.git   "]),
            control(&[
                "#Vcs-Git: git://git.debian.org/collab-maint/<pkg>.git",
                "#Vcs-Browser: http://git.debian.org/?p=collab-maint/<pkg>.git",
            ]),
        ];
        SourceTreeTester::new(ControlFileContainsDhMakeVcsComment, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(control(&["#Vcs-Svn: x"]))];
        let fail = vec![SourcePackageCase::from(control(&[
            "#Vcs-Git: git://git.debian.org/collab-maint/<pkg>.git",
        ]))];
        SourcePackageTester::new(ControlFileContainsDhMakeVcsComment, pass, fail)
            .test_and_snapshot();
    }
}
