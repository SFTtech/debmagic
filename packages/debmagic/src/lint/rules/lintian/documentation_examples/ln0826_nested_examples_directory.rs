use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NestedExamplesDirectory;

declare_rule! {
    /// A directory `usr/share/doc/<package>/examples/examples` is installed.
    ///
    /// Walks the directory entries of Data files. Only directories match;
    /// a file or symlink of that name is not reported. Extra is empty; the
    /// path with a trailing slash is the Location. Udebs are checked too.
    NestedExamplesDirectory,
    code = "LN0826",
    tag = "nested-examples-directory",
    default_selected = true,
    experimental = false,
}

static NESTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/share/doc/[^/]+/examples/examples/?$").expect("valid regex")
});

impl BinaryPackageRule for NestedExamplesDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::Directory))
            .filter_map(|entry| {
                let path = entry.path().to_str()?;
                NESTED
                    .is_match(path)
                    .then(|| PathBuf::from(format!("{path}/")))
            })
            .collect();
        for path in paths {
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
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data("usr/share/doc/pkg/examples", TestFile::directory()),
            data("usr/share/doc/pkg/examples/examples", TestFile::new("x")),
            data(
                "usr/share/doc/pkg/examples/examples",
                TestFile::symlink("x"),
            ),
            data("usr/share/doc/pkg/examples/examples/x", TestFile::new("x")),
            data(
                "usr/share/doc/pkg/examples/examples/sub",
                TestFile::directory(),
            ),
            // Only one package level below usr/share/doc/.
            data("usr/share/doc/examples/examples", TestFile::directory()),
            data("usr/share/doc/a/b/examples/examples", TestFile::directory()),
            data(
                "usr/share/doc/pkg/examples/examplesx",
                TestFile::directory(),
            ),
        ];
        let fail = vec![
            data("usr/share/doc/pkg/examples/examples", TestFile::directory()),
            data("usr/share/doc/a-b/examples/examples", TestFile::directory()).udeb(),
        ];
        BinaryPackageTester::new(NestedExamplesDirectory, pass, fail).test_and_snapshot();
    }
}
