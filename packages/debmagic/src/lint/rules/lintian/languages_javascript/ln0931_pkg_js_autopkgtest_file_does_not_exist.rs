use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct PkgJsAutopkgtestFileDoesNotExist;

declare_rule! {
    /// A file listed in `debian/tests/pkg-js/files` is missing from the
    /// packaging sources, so `pkg-js-autopkgtest` will likely fail.
    ///
    /// Only for a Source whose `Testsuite` field lists `autopkgtest-pkg-nodejs`.
    /// Each non-empty line of the regular file `debian/tests/pkg-js/files`
    /// is trimmed and used as a pattern: each `*` stands for `[^/]*`, and the
    /// pattern must match a whole Patched path, with an optional trailing `/`.
    /// A directory is matched with its trailing `/`, as lintian names it, so
    /// `dir/` matches a directory and `dir` matches it too.
    /// A line that matches nothing is reported with the trimmed line as extra
    /// and no Location. Lintian keeps empty lines too, so an empty line is
    /// reported with an empty extra. A pattern that is not a valid regular
    /// expression after the `*` replacement is skipped, where lintian would
    /// stop.
    PkgJsAutopkgtestFileDoesNotExist,
    code = "LN0931",
    tag = "pkg-js-autopkgtest-file-does-not-exist",
    default_selected = true,
    experimental = false,
}

const FILES_LIST: &str = "debian/tests/pkg-js/files";

impl SourceRule for PkgJsAutopkgtestFileDoesNotExist {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let uses_nodejs_testsuite = ctx
            .debian_control()
            .and_then(|control| control.source().unfolded_value("Testsuite"))
            .is_some_and(|value| {
                value
                    .split(',')
                    .any(|suite| suite.trim() == "autopkgtest-pkg-nodejs")
            });
        if !uses_nodejs_testsuite {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(entry) = files.get(FILES_LIST) else {
            return;
        };
        if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            return;
        }
        let Ok(bytes) = files.read(entry) else {
            return;
        };
        let text = String::from_utf8_lossy(&bytes);
        let missing: Vec<String> = text
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|pattern| !exists(files, pattern))
            .collect();
        for pattern in missing {
            ctx.diagnostic(Diagnostic::error(pattern));
        }
    }
}

/// Whether some Patched path matches the pattern, as lintian's `path_exists`.
fn exists(files: &FileTree, pattern: &str) -> bool {
    let expression = pattern.replace('*', "[^/]*");
    let Ok(regex) = Regex::new(&format!("^{expression}/?$")) else {
        return true;
    };
    files.entries().iter().any(|entry| {
        let Some(path) = entry.path().to_str() else {
            return false;
        };
        if matches!(entry.kind(), EntryKind::Directory) {
            regex.is_match(&format!("{path}/"))
        } else {
            regex.is_match(path)
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\nTestsuite: autopkgtest-pkg-nodejs\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n";

    fn package(files: Vec<(&str, TestFile)>) -> SourcePackageCase {
        let mut patched = vec![("debian/control", TestFile::new(CONTROL))];
        patched.extend(files);
        SourcePackageCase::from(patched)
    }

    fn list(body: &str) -> (&'static str, TestFile) {
        ("debian/tests/pkg-js/files", TestFile::new(body.to_string()))
    }

    #[test]
    fn source() {
        let pass = vec![
            package(vec![
                list("lib/*/*.json\nla/test\n"),
                ("lib/a/b.json", TestFile::new("{}")),
                ("la/test", TestFile::new("x")),
            ]),
            // A directory matches the pattern.
            package(vec![
                list("docs\n"),
                ("docs", TestFile::directory()),
                ("docs/readme", TestFile::new("x")),
            ]),
            // A directory named with its trailing slash, as lintian names it.
            package(vec![
                list("test_modules/\n"),
                ("test_modules", TestFile::directory()),
                ("test_modules/a.js", TestFile::new("x")),
            ]),
            // Without the nodejs Testsuite the list is not checked.
            SourcePackageCase::from(vec![
                (
                    "debian/control",
                    TestFile::new(
                        "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
                    ),
                ),
                list("missing\n"),
            ]),
        ];
        let fail = vec![
            package(vec![list("missing.txt\n")]),
            package(vec![
                list("lib/*/*.json\n"),
                ("lib/a.txt", TestFile::new("x")),
            ]),
        ];
        SourcePackageTester::new(PkgJsAutopkgtestFileDoesNotExist, pass, fail).test_and_snapshot();
    }
}
