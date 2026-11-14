use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PythonModuleHasOverlyGenericName;

declare_rule! {
    /// A Python module in the global namespace uses a generic name.
    ///
    /// `usr/lib/python3/` and `usr/lib/python3.Y/` `dist-packages` and
    /// `site-packages`. The module is `name.py` or `name/__init__.py`, and
    /// `name` matches a pattern from lintian's `files/generic-python-modules`
    /// (`backports?`, `core`, `docs?`, `site`, `tests?`, `examples?`,
    /// `scripts?`), ignoring case. Extra is the matched name in parentheses.
    /// The path is the Location. Udebs are checked too.
    PythonModuleHasOverlyGenericName,
    code = "LN0501",
    tag = "python-module-has-overly-generic-name",
    default_selected = true,
    experimental = false,
}

/// lintian `data/files/generic-python-modules`, in file order.
const GENERIC_PATTERNS: &[&str] = &[
    "backports?",
    "core",
    "docs?",
    "site",
    "tests?",
    "examples?",
    "scripts?",
];

static MODULE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\Ausr/lib/python3(?:\.\d+)?/(?:site|dist)-packages/(.+)\z").expect("valid regex")
});

static GENERIC: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    GENERIC_PATTERNS
        .iter()
        .map(|pattern| {
            Regex::new(&format!(r"(?i)^({pattern})(?:\.py|/__init__\.py)$"))
                .unwrap_or_else(|_| panic!("valid generic python module regex {pattern}"))
        })
        .collect()
});

impl BinaryPackageRule for PythonModuleHasOverlyGenericName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits: Vec<(PathBuf, String)> = Vec::new();
        for entry in files.entries() {
            if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                continue;
            }
            let Some(path) = entry.path().to_str() else {
                continue;
            };
            let Some(relative) = MODULE.captures(path).and_then(|captures| captures.get(1)) else {
                continue;
            };
            for name in generic_names(relative.as_str()) {
                hits.push((entry.path().to_path_buf(), name));
            }
        }
        hits.sort();
        for (path, name) in hits {
            ctx.diagnostic(Diagnostic::error(name).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Matched module names, in data-file order. The capture keeps the path's case.
fn generic_names(relative: &str) -> Vec<String> {
    GENERIC
        .iter()
        .filter_map(|regex| {
            regex
                .captures(relative)
                .and_then(|captures| captures.get(1))
                .map(|name| format!("({})", name.as_str()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| ((*path).to_string(), TestFile::new("x")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            files(&["usr/lib/python3/dist-packages/false_positive/__init__.py"]),
            files(&["usr/lib/python3/dist-packages/tests/foo.py"]),
            files(&["usr/lib/python3/dist-packages/tests/__init__.pyc"]),
            files(&["usr/lib/python2.7/dist-packages/tests.py"]),
            files(&["usr/lib/python3/dist-packages/real_module.py"]),
            files(&["usr/share/doc/foo/examples/__init__.py"]),
            BinaryPackageCase::from(vec![(
                "usr/lib/python3/dist-packages/tests.py".to_string(),
                TestFile::symlink("real.py"),
            )]),
        ];
        let fail = vec![
            files(&[
                "usr/lib/python3/dist-packages/site.py",
                "usr/lib/python3/dist-packages/tests.py",
                "usr/lib/python3/dist-packages/doc/__init__.py",
                "usr/lib/python3/dist-packages/docs/__init__.py",
                "usr/lib/python3/dist-packages/test/__init__.py",
                "usr/lib/python3/dist-packages/tests/__init__.py",
                "usr/lib/python3/dist-packages/example/__init__.py",
                "usr/lib/python3/dist-packages/examples/__init__.py",
                "usr/lib/python3/dist-packages/false_positive/__init__.py",
            ]),
            files(&[
                "usr/lib/python3/dist-packages/core.py",
                "usr/lib/python3/dist-packages/backport.py",
                "usr/lib/python3/dist-packages/backports/__init__.py",
                "usr/lib/python3/dist-packages/script/__init__.py",
                "usr/lib/python3/dist-packages/scripts.py",
                "usr/lib/python3/dist-packages/Tests.PY",
            ]),
            files(&["usr/lib/python3.11/site-packages/docs/__init__.py"]),
            BinaryPackageCase::from(vec![(
                "usr/lib/python3/dist-packages/site.py".to_string(),
                TestFile::new("x").hardlink(),
            )]),
            files(&["usr/lib/python3/dist-packages/core.py"]).udeb(),
        ];
        BinaryPackageTester::new(PythonModuleHasOverlyGenericName, pass, fail).test_and_snapshot();
    }
}
