use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{SourceContext, follow_symlinks};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::{Severity, SourceRule};

pub struct UsesDeprecatedPythonStdlib;

declare_rule! {
    /// A patched `*.py` file imports a standard library module that PEP 594 and
    /// PEP 632 deprecate.
    ///
    /// Only Source packages whose `Build-Depends-All` satisfy a Python 3
    /// package are checked, and files under `.pc/` are skipped. A file that is
    /// not an open file (a symlink out of the tree) is skipped. Each line that
    /// starts with `from` or `import` is matched against `from <module>` and
    /// `import <module>` forms, and relative imports do not count. Reported at
    /// the line with extra `<module> (deprecated in Python <v>, removed in
    /// Python <v>)`. Applies to the Source tree and the Source package.
    UsesDeprecatedPythonStdlib,
    code = "LN1405",
    tag = "uses-deprecated-python-stdlib",
    default_selected = false,
    experimental = false,
}

/// lintian's `%DEPRECATED_STDLIBS`: module, deprecated in, removed in.
const DEPRECATED: &[(&str, &str, &str)] = &[
    ("aifc", "3.11", "3.13"),
    ("asynchat", "3.6", "3.12"),
    ("asyncore", "3.6", "3.12"),
    ("audioop", "3.11", "3.13"),
    ("cgi", "3.11", "3.13"),
    ("cgitb", "3.11", "3.13"),
    ("chunk", "3.11", "3.13"),
    ("crypt", "3.11", "3.13"),
    ("distutils", "3.10", "3.12"),
    ("imghdr", "3.11", "3.13"),
    ("mailcap", "3.11", "3.13"),
    ("msilib", "3.11", "3.13"),
    ("nntplib", "3.11", "3.13"),
    ("nis", "3.11", "3.13"),
    ("ossaudiodev", "3.11", "3.13"),
    ("pipes", "3.11", "3.13"),
    ("smtpd", "3.5.4", "3.12"),
    ("sndhdr", "3.11", "3.13"),
    ("spwd", "3.11", "3.13"),
    ("sunau", "3.11", "3.13"),
    ("telnetlib", "3.11", "3.13"),
    ("uu", "3.11", "3.13"),
    ("xdrlib", "3.11", "3.13"),
];

const PYTHON3: &str = "python3:any | python3-dev:any | python3-all:any | python3-all-dev:any";

struct Matchers {
    module: &'static str,
    deprecated: &'static str,
    removed: &'static str,
    from: Regex,
    import: Regex,
}

static MATCHERS: LazyLock<Vec<Matchers>> = LazyLock::new(|| {
    DEPRECATED
        .iter()
        .map(|&(module, deprecated, removed)| Matchers {
            module,
            deprecated,
            removed,
            from: Regex::new(&format!(r"^\s*from {module}(\s+|\..+)import")).expect("valid regex"),
            import: Regex::new(&format!(r"^\s*import(\s+|.+,\s?){module}([,.\s]|$)"))
                .expect("valid regex"),
        })
        .collect()
});

impl SourceRule for UsesDeprecatedPythonStdlib {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let build_all = match ctx.debian_control() {
            Some(control) => control.source().relations_and(&[
                "Build-Depends",
                "Build-Depends-Indep",
                "Build-Depends-Arch",
            ]),
            None => return,
        };
        if !build_all_satisfies_python3(&build_all) {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let mut findings: Vec<(PathBuf, u32, String)> = Vec::new();
        for entry in files.entries() {
            let bytes = entry.path().as_os_str().as_encoded_bytes();
            if !bytes.ends_with(b".py") || bytes.starts_with(b".pc/") {
                continue;
            }
            let Some(target) = follow_symlinks(files, entry) else {
                continue;
            };
            let Ok(content) = files.read(target) else {
                continue;
            };
            for (index, line) in content.split(|byte| *byte == b'\n').enumerate() {
                let line = String::from_utf8_lossy(line);
                if !line.trim_start().starts_with("from")
                    && !line.trim_start().starts_with("import")
                {
                    continue;
                }
                for matcher in MATCHERS.iter() {
                    if matcher.from.is_match(&line) || matcher.import.is_match(&line) {
                        findings.push((
                            entry.path().to_path_buf(),
                            index as u32 + 1,
                            format!(
                                "{} (deprecated in Python {}, removed in Python {})",
                                matcher.module, matcher.deprecated, matcher.removed
                            ),
                        ));
                    }
                }
            }
        }
        for (path, line, message) in findings {
            ctx.diagnostic(
                Diagnostic::warn(message)
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

fn build_all_satisfies_python3(build_all: &PackageRelation) -> bool {
    build_all.satisfies(PYTHON3)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(
        build_depends: &str,
        files: Vec<(&'static str, &str)>,
    ) -> Vec<(&'static str, TestFile)> {
        let mut tree = vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nBuild-Depends: {build_depends}\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )];
        tree.extend(
            files
                .into_iter()
                .map(|(path, content)| (path, TestFile::new(content))),
        );
        tree
    }

    #[test]
    fn test() {
        let pass = vec![
            // No Python dependency.
            tree("debhelper", vec![("example.py", "import asyncore\n")]),
            // Not a `.py` file.
            tree("python3", vec![("example.txt", "import asyncore\n")]),
            // Relative import and similar module names.
            tree(
                "python3",
                vec![(
                    "example.py",
                    "from .asyncore import x\nimport asyncore2\nfrom notasyncore import y\nimport notasyncore\n",
                )],
            ),
            // Under `.pc/` (quilt).
            tree("python3", vec![(".pc/fix/example.py", "import asyncore\n")]),
        ];
        let fail = vec![
            tree("python3", vec![("example.py", "import asyncore\n")]),
            tree(
                "python3-dev",
                vec![("example.py", "from asyncore import x\n")],
            ),
            tree(
                "python3",
                vec![(
                    "example.py",
                    "import os, distutils.core\nimport foo, asynchat\n",
                )],
            ),
            tree(
                "python3-all",
                vec![("sub/example.py", "    import pipes as p\n")],
            ),
        ];
        SourceTreeTester::new(UsesDeprecatedPythonStdlib, pass, fail).test_and_snapshot();
    }
}
