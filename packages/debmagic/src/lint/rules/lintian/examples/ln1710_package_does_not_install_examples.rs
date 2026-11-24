use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::group_context::GroupContext;
use crate::lint::rule::{GroupRule, Severity};

pub struct PackageDoesNotInstallExamples;

declare_rule! {
    /// The source has a file or directory named `examples`, but no package of
    /// the Group installs examples.
    ///
    /// The Group ships examples when one of its installables is named
    /// `*-examples`, or when an installable's installed files contain
    /// `usr/share/doc/<anything>/examples` (the directory itself, with or
    /// without a trailing slash). Every entry of the patched source named
    /// `examples` is reported, except under `.pc/` and under a `vendor` or
    /// `third_party` directory. Nothing is reported for a Group with no
    /// installables. Reported at pedantic severity, with the entry path as the
    /// Location (a directory with a trailing slash, as lintian's pointer) and
    /// an empty extra. A Group Rule: the finding is on the source
    /// package. Udebs count as installables, as lintian's `get_installables`.
    ///
    /// Where we differ from lintian: none for the Rule itself. Lintian's
    /// `Screen/Examples/InTests.pm` masks the hint for an entry under a
    /// top-level `test/` or `tests/` directory of the source (test fixtures,
    /// not examples), and we do the same; a nested `test` directory is reported.
    PackageDoesNotInstallExamples,
    code = "LN1710",
    tag = "package-does-not-install-examples",
    default_selected = false,
    experimental = false,
}

static SHIPPED_EXAMPLES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/doc/(.+/)?examples/?$").expect("valid regex"));
static VENDORED_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|/)(?:vendor|third_party)/").expect("valid regex"));

/// lintian's `Screen/Examples/InTests.pm`: an item named `test/...` or
/// `tests/...`, with the directory at the top of the source.
fn in_top_level_test_directory(path: &Path) -> bool {
    let mut parts = path.components();
    matches!(parts.next(), Some(Component::Normal(dir)) if dir == "test" || dir == "tests")
        && parts.next().is_some()
}

impl GroupRule for PackageDoesNotInstallExamples {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let installables = ctx.installables();
        if installables.is_empty() {
            return;
        }
        if installables
            .iter()
            .any(|installable| installable.name().ends_with("-examples"))
        {
            return;
        }
        let mut ships_examples = false;
        for installable in &installables {
            let Some(files) = ctx.data_files(installable) else {
                continue;
            };
            if files
                .entries()
                .iter()
                .any(|entry| SHIPPED_EXAMPLES.is_match(&entry.path().to_string_lossy()))
            {
                ships_examples = true;
                break;
            }
        }
        if ships_examples {
            return;
        }
        let Some(source) = ctx.patched_files() else {
            return;
        };
        let paths: Vec<PathBuf> = source
            .entries()
            .iter()
            .filter(|entry| {
                entry
                    .path()
                    .file_name()
                    .is_some_and(|name| name == "examples")
            })
            .filter(|entry| !entry.path().starts_with(".pc"))
            .filter(|entry| !in_top_level_test_directory(entry.path()))
            .filter(|entry| {
                let dir = entry
                    .path()
                    .parent()
                    .map(|parent| format!("{}/", parent.to_string_lossy()))
                    .unwrap_or_default();
                !VENDORED_DIR.is_match(&dir)
            })
            .map(|entry| {
                // lintian names a directory with a trailing slash in its pointer.
                if entry.is_dir() {
                    PathBuf::from(format!("{}/", entry.path().display()))
                } else {
                    entry.path().to_path_buf()
                }
            })
            .collect();
        for path in paths {
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

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester, TestCase, TestFile};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";

    fn source(paths: &[&'static str]) -> TestCase {
        TestCase::from(
            paths
                .iter()
                .map(|path| (*path, TestFile::directory()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn group() {
        let pass = vec![
            // The Group has a `-examples` package.
            GroupCase::new(DSC)
                .installable("foo", "")
                .files([("usr/bin/foo", "x")])
                .installable("foo-examples", "")
                .files([("usr/share/doc/foo-examples/readme", "x")])
                .source_files(source(&["examples"])),
            // An installable ships the examples directory.
            GroupCase::new(DSC)
                .installable("foo", "")
                .files(vec![("usr/share/doc/foo/examples", TestFile::directory())])
                .source_files(source(&["examples"])),
            // Vendored and Debian-internal copies are not reported; no examples in source.
            GroupCase::new(DSC)
                .installable("foo", "")
                .files([("usr/bin/foo", "x")])
                .source_files(source(&[
                    "vendor/examples",
                    "third_party/x/examples",
                    ".pc/examples",
                ])),
            GroupCase::new(DSC)
                .installable("foo", "")
                .files([("usr/bin/foo", "x")]),
            // No installables: nothing to compare with.
            GroupCase::new(DSC).source_files(source(&["examples"])),
        ];
        let fail = vec![
            GroupCase::new(DSC)
                .installable("foo", "")
                .files([("usr/bin/foo", "x")])
                .source_files(source(&["examples"])),
            GroupCase::new(DSC)
                .installable("foo", "")
                .files([("usr/share/doc/foo/other", "x")])
                .source_files(source(&["docs/examples", "examples"])),
        ];
        GroupTester::new(PackageDoesNotInstallExamples, pass, fail).test_and_snapshot();
    }

    /// lintian `Screen/Examples/InTests.pm` masks the hint for an item whose name
    /// starts with `test/` or `tests/` at the top of the source: test fixtures are
    /// not examples of the package. Only the top-level directory counts, so a
    /// nested `test` folder, a similar name such as `testing/`, and an `examples`
    /// entry outside the tests are still reported.
    #[test]
    fn examples_under_a_top_level_test_directory_are_masked() {
        let pass = vec![
            GroupCase::new(DSC)
                .installable("foo", "")
                .files([("usr/bin/foo", "x")])
                .source_files(source(&[
                    "test/examples",
                    "tests/examples",
                    "tests/x/examples",
                ])),
        ];
        let fail = vec![
            // Nested under another directory: not the top-level test folder.
            GroupCase::new(DSC)
                .installable("foo", "")
                .files([("usr/bin/foo", "x")])
                .source_files(source(&["src/test/examples"])),
            // A similar name is not the test folder.
            GroupCase::new(DSC)
                .installable("foo", "")
                .files([("usr/bin/foo", "x")])
                .source_files(source(&["testing/examples", "test-data/examples"])),
            // The unmasked neighbour in the same source is still reported.
            GroupCase::new(DSC)
                .installable("foo", "")
                .files([("usr/bin/foo", "x")])
                .source_files(source(&["test/examples", "examples"])),
        ];
        GroupTester::new(PackageDoesNotInstallExamples, pass, fail).test();
    }
}
