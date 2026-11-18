use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{Severity, SourceRule};

pub struct AbsoluteSymbolicLinkTargetInSource;

declare_rule! {
    /// A symlink in the packaging File tree has an absolute target.
    ///
    /// Walks the Source tree or Source package Patched files symlinks. A
    /// target that starts with `/` is reported, except the exact target
    /// `/dev/null` (masked systemd units). Orig files are not checked. Extra
    /// is the link text as written; the symlink's path is the Location.
    AbsoluteSymbolicLinkTargetInSource,
    code = "LN0733",
    tag = "absolute-symbolic-link-target-in-source",
    default_selected = true,
    experimental = false,
}

impl SourceRule for AbsoluteSymbolicLinkTargetInSource {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let found: Vec<(std::path::PathBuf, String)> = files
            .query()
            .kinds([EntryKind::Symlink])
            .into_iter()
            .filter_map(|entry| {
                let target = entry.link_target()?.to_string_lossy().into_owned();
                (target.starts_with('/') && target != "/dev/null")
                    .then(|| (entry.path().to_path_buf(), target))
            })
            .collect();
        for (path, link) in found {
            ctx.diagnostic(
                Diagnostic::error(link)
                    .with_severity(Severity::Warning)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
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
            tree(&[("link", TestFile::symlink("../target"))]),
            tree(&[("link", TestFile::symlink("target"))]),
            tree(&[("systemd.service", TestFile::symlink("/dev/null"))]),
            tree(&[("dir", TestFile::directory())]),
            tree(&[("file", TestFile::new("/usr/share/x"))]),
        ];
        let fail = vec![
            tree(&[("link", TestFile::symlink("/usr/share/x"))]),
            tree(&[("link", TestFile::symlink("/"))]),
            tree(&[("link", TestFile::symlink("/dev/null/"))]),
            tree(&[("link", TestFile::symlink("/dev/nullx"))]),
            tree(&[
                ("debian/link", TestFile::symlink("/usr/share/x")),
                ("src/link", TestFile::symlink("/usr/share/y")),
            ]),
        ];
        SourceTreeTester::new(AbsoluteSymbolicLinkTargetInSource, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(tree(&[])),
            SourcePackageCase::from(tree(&[("link", TestFile::symlink("../target"))])),
            SourcePackageCase::from(tree(&[("link", TestFile::symlink("/dev/null"))])),
            SourcePackageCase::from(tree(&[])).orig(vec![(
                "orig-link".to_string(),
                TestFile::symlink("/usr/share/x"),
            )]),
        ];
        let fail = vec![SourcePackageCase::from(tree(&[(
            "link",
            TestFile::symlink("/usr/share/x"),
        )]))];
        SourcePackageTester::new(AbsoluteSymbolicLinkTargetInSource, pass, fail)
            .test_and_snapshot();
    }
}
