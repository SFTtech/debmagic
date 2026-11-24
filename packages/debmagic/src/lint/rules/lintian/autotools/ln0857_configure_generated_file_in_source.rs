use std::path::PathBuf;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct ConfigureGeneratedFileInSource;

declare_rule! {
    /// The Source tree or Source package contains a `config.cache`,
    /// `config.log` or `config.status` file produced by GNU autoconf's
    /// `configure`.
    ///
    /// Regular files and hardlinks (not directories or symlinks) whose basename
    /// matches `config.(cache|log|status)` are hits, outside `debian/`. Like
    /// lintian, the `.` in the basename is any character. Quilt `.pc/` is
    /// skipped, as lintian skips it for patched files. Extra is empty; the path
    /// is the Location (lintian's `pointed_hint` with the item pointer, printed
    /// `[tests/config.cache]`). The vendored `cruft-general-upstream` recipe is
    /// a lintian `Todo` test with a stale bare `config.cache` line; Parity
    /// rewrites our hint for that recipe only. Warning severity, from the
    /// `.tag` file.
    ConfigureGeneratedFileInSource,
    code = "LN0857",
    tag = "configure-generated-file-in-source",
    default_selected = true,
    experimental = false,
}

impl SourceRule for ConfigureGeneratedFileInSource {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let pattern = Regex::new(r"^config.(?:cache|log|status)$").expect("valid regex");
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter(|entry| !entry.path().starts_with("debian"))
            .filter(|entry| !entry.path().starts_with(".pc"))
            .filter(|entry| {
                entry
                    .path()
                    .file_name()
                    .is_some_and(|name| pattern.is_match(&name.to_string_lossy()))
            })
            .map(|entry| entry.path().to_path_buf())
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
            tree(&[("config.guess", TestFile::new("x"))]),
            tree(&[("configure.ac", TestFile::new("x"))]),
            tree(&[("config.cache.bak", TestFile::new("x"))]),
            tree(&[("myconfig.cache", TestFile::new("x"))]),
            tree(&[("config.cache", TestFile::directory())]),
            tree(&[("config.cache", TestFile::symlink("x"))]),
            // Anything under debian/ is skipped, as lintian does.
            tree(&[("debian/config.cache", TestFile::new("x"))]),
            tree(&[("debian/src/config.status", TestFile::new("x"))]),
        ];
        let fail = vec![
            tree(&[("config.cache", TestFile::new("x"))]),
            tree(&[("config.log", TestFile::new("x"))]),
            tree(&[("config.status", TestFile::new("x"))]),
            tree(&[("tests/config.cache", TestFile::new("x"))]),
            tree(&[
                ("src/config.status", TestFile::new("x")),
                ("config.log", TestFile::new("x")),
            ]),
            tree(&[("config.cache", TestFile::new("x").hardlink())]),
        ];
        SourceTreeTester::new(ConfigureGeneratedFileInSource, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(tree(&[])),
            // Quilt patches are not searched.
            SourcePackageCase::from(tree(&[(".pc/fix.patch/config.cache", TestFile::new("x"))])),
        ];
        let fail = vec![SourcePackageCase::from(tree(&[(
            "config.status",
            TestFile::new("x"),
        )]))];
        SourcePackageTester::new(ConfigureGeneratedFileInSource, pass, fail).test_and_snapshot();
    }
}
