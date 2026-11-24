use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct DhMakeTemplateInSource;

declare_rule! {
    /// A `dh_make` template is still in the packaging `debian/` directory.
    ///
    /// Visits every Entry of the packaging File tree (Source tree, or Source
    /// package Patched files) whose parent is exactly `debian` and whose file
    /// name starts with `ex.` or ends with `.ex`, ignoring case. Each one is
    /// reported with an empty extra and the Entry's path as the Location, with
    /// no line. Directories and symlinks match too, as in lintian, which does
    /// not check the file type here. Entries in subdirectories of `debian/`
    /// are not checked. Paths keep their raw bytes, so a non-UTF-8 name is
    /// matched on its bytes.
    DhMakeTemplateInSource,
    code = "LN1278",
    tag = "dh-make-template-in-source",
    default_selected = true,
    experimental = false,
}

static TEMPLATE_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i-u)^ex\.|\.ex$").expect("valid regex"));

impl SourceRule for DhMakeTemplateInSource {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let hits: Vec<PathBuf> = files
            .entries()
            .iter()
            .map(|entry| entry.path())
            .filter(|path| path.parent() == Some(Path::new("debian")))
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| TEMPLATE_NAME.is_match(name.as_encoded_bytes()))
            })
            .map(Path::to_path_buf)
            .collect();
        for path in hits {
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

    /// A packaging tree with `debian/control` and the given extra files.
    fn tree(extra: Vec<(&str, TestFile)>) -> Vec<(String, TestFile)> {
        let mut out = vec![(
            "debian/control".to_string(),
            TestFile::new("Source: example\n"),
        )];
        out.extend(
            extra
                .into_iter()
                .map(|(path, file)| (path.to_string(), file)),
        );
        out
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            tree(vec![]),
            tree(vec![("debian/example.conf", TestFile::new("x\n"))]),
            tree(vec![("debian/foo.exe", TestFile::new("x\n"))]),
            // Only direct children of debian/ are templates.
            tree(vec![("debian/sub/foo.ex", TestFile::new("x\n"))]),
            tree(vec![("usr/share/foo.ex", TestFile::new("x\n"))]),
        ];
        let fail = vec![
            tree(vec![("debian/postinst.ex", TestFile::new("x\n"))]),
            tree(vec![("debian/ex.postinst", TestFile::new("x\n"))]),
            // Case-insensitive.
            tree(vec![("debian/Manpage.1.EX", TestFile::new("x\n"))]),
            // A symlink is still an Entry with that name.
            tree(vec![("debian/init.d.ex", TestFile::symlink("elsewhere"))]),
            // A hardlink is an Entry with that name.
            tree(vec![("debian/watch.ex", TestFile::new("x\n").hardlink())]),
        ];
        SourceTreeTester::new(DhMakeTemplateInSource, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::new("Source: example\n"),
        )])];
        let fail = vec![SourcePackageCase::from(vec![(
            "debian/cron.d.ex",
            TestFile::new("x\n"),
        )])];
        SourcePackageTester::new(DhMakeTemplateInSource, pass, fail).test_and_snapshot();
    }
}
