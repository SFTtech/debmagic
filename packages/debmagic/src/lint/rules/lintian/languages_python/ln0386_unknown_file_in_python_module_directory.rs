use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct UnknownFileInPythonModuleDirectory;

declare_rule! {
    /// A top-level file in a Python module directory is not a module, extension,
    /// or package metadata.
    ///
    /// `usr/lib/python3/` and `usr/lib/python3.Y/` `dist-packages` and
    /// `site-packages`. Allowed names end in `.egg-info`, `.so`, `.pth`, or
    /// `.py` / `.pyc` / `.pyi`, ignoring case, except `__init__.py`,
    /// `__init__.pyc`, and `__init__.pyi`. Nested files are not this tag.
    /// Extra is empty. The Location is the file.
    UnknownFileInPythonModuleDirectory,
    code = "LN0386",
    tag = "unknown-file-in-python-module-directory",
    default_selected = true,
    experimental = false,
}

static MODULE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib/python(3(?:\.\d+)?)/(?:site|dist)-packages/([^/]+)$")
        .expect("valid regex")
});
static ALLOWED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:\.egg-info|\.so|\.pth|\.py[ic]?)$").expect("valid regex"));
static INIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^__init__\.py[ic]?$").expect("valid regex"));

impl BinaryPackageRule for UnknownFileInPythonModuleDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter_map(|entry| {
                let path = entry.path().to_str()?;
                let captures = MODULE.captures(path)?;
                let basename = captures.get(2)?.as_str();
                if !ALLOWED.is_match(basename) || INIT.is_match(basename) {
                    Some(entry.path().to_path_buf())
                } else {
                    None
                }
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

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("x")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            files(&["usr/lib/python3/dist-packages/foo.py"]),
            files(&["usr/lib/python3/dist-packages/foo.so"]),
            files(&["usr/lib/python3/dist-packages/Foo.PY"]),
            files(&["usr/lib/python3/dist-packages/pkg/__init__.py"]),
            files(&["usr/lib/python2.7/dist-packages/bad"]),
        ];
        let fail = vec![
            files(&[
                "usr/lib/python3/dist-packages/bad",
                "usr/lib/python3/dist-packages/__init__.py",
            ]),
            files(&["usr/lib/python3.2/site-packages/python-foo"]),
            files(&["usr/lib/python3/dist-packages/mod.pyc.bak"]),
        ];
        BinaryPackageTester::new(UnknownFileInPythonModuleDirectory, pass, fail)
            .test_and_snapshot();
    }
}
