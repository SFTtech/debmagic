use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Entry;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct ExecutableInUsrLib;

declare_rule! {
    /// An executable file is installed under `usr/lib/`.
    ///
    /// Walks Data files. A file or hardlink under `usr/lib/` with any execute
    /// bit set is reported; FHS places such executables in `usr/libexec/`.
    /// Symlinks and directories are not reported, and the Tag is skipped for
    /// udebs. Lintian's per-screen exemptions (emacsen, CGI) are not modelled,
    /// so those packages are reported too. Extra is empty; the Location is the
    /// file.
    ExecutableInUsrLib,
    code = "LN1723",
    tag = "executable-in-usr-lib",
    default_selected = false,
    experimental = true,
}

impl BinaryPackageRule for ExecutableInUsrLib {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| is_executable_in_usr_lib(entry))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

fn is_executable_in_usr_lib(entry: &Entry) -> bool {
    entry.path().starts_with("usr/lib") && entry.is_executable_file()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn exec(content: &str) -> TestFile {
        TestFile::new(content).mode(0o755)
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(vec![]),
            // Executables belong in usr/libexec.
            case(vec![("usr/libexec/foo".to_string(), exec("x"))]),
            // Not executable.
            case(vec![(
                "usr/lib/libfoo.so.1".to_string(),
                TestFile::new("x").mode(0o644),
            )]),
            // Directories are not files, even with the execute bit.
            case(vec![(
                "usr/lib/foo".to_string(),
                TestFile::directory().mode(0o755),
            )]),
            // A symlink is not a file.
            case(vec![("usr/lib/foo".to_string(), TestFile::symlink("bar"))]),
            // Only usr/lib, not a similar prefix like usr/library.
            case(vec![("usr/library/foo".to_string(), exec("x"))]),
            case(vec![("lib/foo".to_string(), exec("x"))]),
            // Udebs are skipped.
            case(vec![("usr/lib/foo".to_string(), exec("x"))]).udeb(),
        ];
        let fail = vec![
            case(vec![("usr/lib/some-executable".to_string(), exec("x"))]),
            // A hardlink counts as a file, so it is reported too.
            case(vec![
                ("usr/lib/foo".to_string(), exec("x")),
                (
                    "usr/lib/bar".to_string(),
                    TestFile::new("x").mode(0o755).hardlink_to("usr/lib/foo"),
                ),
            ]),
            // Nested directories too.
            case(vec![("usr/lib/foo/bar/baz".to_string(), exec("x"))]),
        ];
        BinaryPackageTester::new(ExecutableInUsrLib, pass, fail).test_and_snapshot();
    }
}
