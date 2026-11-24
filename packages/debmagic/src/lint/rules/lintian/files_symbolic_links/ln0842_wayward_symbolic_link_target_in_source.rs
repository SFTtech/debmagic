use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;
use crate::lint::rules::lintian::documentation_texinfo::scan::normalize_link_target;

pub struct WaywardSymbolicLinkTargetInSource;

declare_rule! {
    /// A relative symlink in the packaging File tree leaves the source tree.
    ///
    /// Walks the Source tree or Source package Patched files symlinks. A
    /// relative target (not starting with `/`) is reported when it resolved
    /// against the link's directory climbs above the top of the tree. Absolute
    /// targets are `absolute-symbolic-link-target-in-source`. Orig files are
    /// not checked. Extra is the link text as written; the symlink's path is
    /// the Location.
    WaywardSymbolicLinkTargetInSource,
    code = "LN0842",
    tag = "wayward-symbolic-link-target-in-source",
    default_selected = true,
    experimental = false,
}

impl SourceRule for WaywardSymbolicLinkTargetInSource {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let found: Vec<(PathBuf, String)> = files
            .query()
            .kinds([EntryKind::Symlink])
            .into_iter()
            .filter_map(|entry| {
                let target = entry.link_target()?.to_string_lossy().into_owned();
                if target.starts_with('/') {
                    return None;
                }
                let dir = entry
                    .path()
                    .parent()
                    .map(|parent| parent.to_string_lossy().into_owned())
                    .unwrap_or_default();
                normalize_link_target(&dir, &target)
                    .is_none()
                    .then(|| (entry.path().to_path_buf(), target))
            })
            .collect();
        for (path, link) in found {
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
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn tree(extra: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        let mut files = vec![("debian/control".to_string(), TestFile::from(CONTROL))];
        files.extend(
            extra
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone())),
        );
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&[("link", TestFile::symlink("target"))]),
            tree(&[("link", TestFile::symlink("."))]),
            tree(&[("link", TestFile::symlink("./x/../y"))]),
            tree(&[("dir/link", TestFile::symlink("../target"))]),
            tree(&[("a/b/link", TestFile::symlink("../../x"))]),
            tree(&[("systemd.service", TestFile::symlink("/dev/null"))]),
            tree(&[("link", TestFile::symlink("/usr/share/x"))]),
            tree(&[("dir", TestFile::directory())]),
        ];
        let fail = vec![
            tree(&[("link", TestFile::symlink("../target"))]),
            tree(&[("link", TestFile::symlink("../../non-existent/path"))]),
            tree(&[("dir/link", TestFile::symlink("../../target"))]),
            tree(&[("a/b/link", TestFile::symlink("../../../x"))]),
            tree(&[
                ("debian/link", TestFile::symlink("../../x")),
                ("src/link", TestFile::symlink("../z")),
            ]),
        ];
        SourceTreeTester::new(WaywardSymbolicLinkTargetInSource, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(tree(&[])),
            SourcePackageCase::from(tree(&[("link", TestFile::symlink("target"))])).orig(vec![(
                "orig-link".to_string(),
                TestFile::symlink("../../x"),
            )]),
        ];
        let fail = vec![SourcePackageCase::from(tree(&[(
            "link",
            TestFile::symlink("../target"),
        )]))];
        SourcePackageTester::new(WaywardSymbolicLinkTargetInSource, pass, fail).test_and_snapshot();
    }
}
