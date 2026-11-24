use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct ExecutableManualPage;

declare_rule! {
    /// A regular file or hardlink under `usr/share/man/` with an execute bit.
    ///
    /// The name must match `usr/share/man/\S+`. Any execute bit (`0o111`)
    /// counts. Symlinks and directories are not checked. Extra is empty. The
    /// Location is the file. Applies to deb, udeb, and ddeb.
    ExecutableManualPage,
    code = "LN0899",
    tag = "executable-manual-page",
    default_selected = true,
    experimental = false,
}

static MAN_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/man/\S+").expect("valid regex"));

impl BinaryPackageRule for ExecutableManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits: Vec<_> = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            files
                .entries()
                .iter()
                .filter(|entry| entry.is_executable_file())
                .filter(|entry| {
                    entry
                        .path()
                        .to_str()
                        .is_some_and(|name| MAN_NAME.is_match(name))
                })
                .map(|entry| entry.path().to_path_buf())
                .collect()
        };
        for path in hits {
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

    fn one(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), file)])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            one(
                "usr/share/man/man1/foo.1.gz",
                TestFile::new("x").mode(0o644),
            ),
            // Executable, but not under usr/share/man.
            one("usr/bin/foo", TestFile::new("x").mode(0o755)),
            // A directory is not a page.
            one("usr/share/man/man1", TestFile::directory().mode(0o755)),
            // A symlink is not a regular file.
            one("usr/share/man/man1/bar.1.gz", TestFile::symlink("foo.1.gz")),
        ];
        let fail = vec![
            one(
                "usr/share/man/man5/foo.5.gz",
                TestFile::new("x").mode(0o755),
            ),
            one(
                "usr/share/man/man5/foo.5.gz",
                TestFile::new("x").mode(0o700),
            ),
            one(
                "usr/share/man/man5/foo.5.gz",
                TestFile::new("x").mode(0o601),
            ),
        ];
        BinaryPackageTester::new(ExecutableManualPage, pass, fail).test_and_snapshot();
    }
}
