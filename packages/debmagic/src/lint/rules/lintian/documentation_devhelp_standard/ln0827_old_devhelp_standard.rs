use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct OldDevhelpStandard;

declare_rule! {
    /// An installed Devhelp index file is named `*.devhelp` or `*.devhelp.gz`.
    ///
    /// The Devhelp index format version 1 is deprecated, and lintian only
    /// judges the file name: any Data entry whose path ends in `.devhelp` or
    /// `.devhelp.gz` is flagged, directories and symlinks included. Extra is
    /// empty; the path is the Location. Udebs are checked too.
    OldDevhelpStandard,
    code = "LN0827",
    tag = "old-devhelp-standard",
    default_selected = true,
    experimental = false,
}

static DEVHELP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)[.]devhelp(?:[.]gz)?$").expect("valid regex"));

impl BinaryPackageRule for OldDevhelpStandard {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| DEVHELP.is_match(entry.path().as_os_str().as_encoded_bytes()))
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
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("content")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // The version 2 index and other names that merely contain `devhelp`.
            files(&[
                "usr/share/devhelp/books/example/index.devhelp2",
                "usr/share/devhelp/books/example/index.devhelp2.gz",
                "usr/share/doc/example/devhelp",
                "usr/share/doc/example/devhelp.txt",
                "usr/share/doc/example/devhelp.gz.txt",
                "usr/share/doc/example/devhelpfoo",
            ]),
        ];
        let fail = vec![
            files(&["usr/share/devhelp/books/example/index.devhelp"]),
            files(&["usr/share/devhelp/books/example/index.devhelp.gz"]),
            files(&["usr/share/doc/example/manual.devhelp"]).udeb(),
            // Directories and symlinks are installed items too.
            BinaryPackageCase::from(vec![(
                "usr/share/devhelp/books/example.devhelp",
                TestFile::directory(),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/devhelp/books/example.devhelp",
                TestFile::symlink("index.devhelp2"),
            )]),
        ];
        BinaryPackageTester::new(OldDevhelpStandard, pass, fail).test_and_snapshot();
    }
}
