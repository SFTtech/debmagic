use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct SourceContainsDebianSubstvars;

declare_rule! {
    /// The Source tree or Source package ships a substvars file under `debian/`.
    ///
    /// Walks the packaging File tree (Source tree, or Source package Patched
    /// files) for regular files and hardlinks named `debian/substvars` or
    /// `debian/<anything>.substvars`; lintian lets `<anything>` span
    /// directories and needs at least one character before the dot. These
    /// files are build products. Extra is empty; the path is the Location.
    SourceContainsDebianSubstvars,
    code = "LN0015",
    tag = "source-contains-debian-substvars",
    default_selected = true,
    experimental = false,
}

static SUBSTVARS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^debian/(?:.+\.)?substvars$").expect("valid regex"));

impl SourceRule for SourceContainsDebianSubstvars {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .map(|entry| entry.path().to_path_buf())
            .filter(|path| path.to_str().is_some_and(|name| SUBSTVARS.is_match(name)))
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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
            // Only under `debian/`.
            tree(&[("substvars", TestFile::new(""))]),
            tree(&[("src/example.substvars", TestFile::new(""))]),
            tree(&[("src/debian/substvars", TestFile::new(""))]),
            // The name must end in `substvars` after a dot.
            tree(&[("debian/example.substvars.bak", TestFile::new(""))]),
            tree(&[("debian/examplesubstvars", TestFile::new(""))]),
            tree(&[("debian/substvars.in", TestFile::new(""))]),
            // A dot alone does not make a name.
            tree(&[("debian/.substvars", TestFile::new(""))]),
            // Directories and symlinks are not files.
            tree(&[("debian/substvars", TestFile::directory())]),
            tree(&[("debian/example.substvars", TestFile::symlink("elsewhere"))]),
        ];
        let fail = vec![
            tree(&[("debian/substvars", TestFile::new("misc:Depends=foo\n"))]),
            tree(&[("debian/example.substvars", TestFile::new(""))]),
            tree(&[(
                "debian/example.postinst.debhelper.substvars",
                TestFile::new(""),
            )]),
            // `.+` spans directories.
            tree(&[("debian/sub/dir.substvars", TestFile::new(""))]),
            tree(&[("debian/a..substvars", TestFile::new(""))]),
            tree(&[
                ("debian/substvars", TestFile::new("")),
                ("debian/somepackage.substvars", TestFile::new("")),
            ]),
            tree(&[(
                "debian/example.substvars",
                TestFile::new("shared").hardlink(),
            )]),
        ];
        SourceTreeTester::new(SourceContainsDebianSubstvars, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(vec![("debian/control", TestFile::from(CONTROL))]),
            SourcePackageCase::from(vec![("debian/substvars.in", TestFile::new(""))]),
        ];
        let fail = vec![
            SourcePackageCase::from(vec![("debian/substvars", TestFile::new(""))]),
            SourcePackageCase::from(vec![(
                "debian/example.substvars",
                TestFile::new("misc:Depends=foo\n"),
            )]),
        ];
        SourcePackageTester::new(SourceContainsDebianSubstvars, pass, fail).test_and_snapshot();
    }
}
