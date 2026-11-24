use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct DebianUpstreamObsoletePath;

declare_rule! {
    /// Upstream metadata is stored at an obsolete path.
    ///
    /// `debian/upstream` or `debian/upstream-metadata.yaml` is a regular file
    /// or hardlink in the packaging File tree (Source tree, or Source package
    /// Patched files). Symlinks and directories are not reported. Extra is
    /// empty; the path is the Location.
    DebianUpstreamObsoletePath,
    code = "LN0845",
    tag = "debian-upstream-obsolete-path",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianUpstreamObsoletePath {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let found: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter(|entry| {
                entry.path() == Path::new("debian/upstream")
                    || entry.path() == Path::new("debian/upstream-metadata.yaml")
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in found {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
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
            tree(&[("debian/upstream/metadata", TestFile::new("Name: x\n"))]),
            tree(&[("debian/upstream", TestFile::directory())]),
            tree(&[("debian/upstream-metadata.yaml", TestFile::directory())]),
            tree(&[("debian/upstream", TestFile::symlink("upstream/metadata"))]),
            tree(&[("debian/upstream.yaml", TestFile::new("Name: x\n"))]),
            tree(&[("upstream", TestFile::new("Name: x\n"))]),
        ];
        let fail = vec![
            tree(&[("debian/upstream", TestFile::new("Name: x\n"))]),
            tree(&[("debian/upstream-metadata.yaml", TestFile::new("Name: x\n"))]),
            tree(&[
                ("debian/upstream", TestFile::new("Name: x\n")),
                ("debian/upstream-metadata.yaml", TestFile::new("Name: x\n")),
            ]),
            tree(&[("debian/upstream", TestFile::new("Name: x\n").hardlink())]),
        ];
        SourceTreeTester::new(DebianUpstreamObsoletePath, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(&[(
            "debian/upstream/metadata",
            TestFile::new("Name: x\n"),
        )]))];
        let fail = vec![SourcePackageCase::from(tree(&[(
            "debian/upstream",
            TestFile::new("Name: x\n"),
        )]))];
        SourcePackageTester::new(DebianUpstreamObsoletePath, pass, fail).test_and_snapshot();
    }
}
