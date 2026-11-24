use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::source_missing::{is_prebuilt_python, visited_files};

pub struct SourceContainsPrebuiltPythonObject;

declare_rule! {
    /// The Source tree or Source package contains a compiled Python file
    /// (`.pyc`, `.pyo`, or a `__pycache__` file).
    ///
    /// A file is reported when it starts with the magic number of a CPython
    /// code object file (releases 1.5 to 3.14) followed by `\r\n`, standing in
    /// for `file(1)`'s "python N byte-compiled"; the name does not matter.
    /// Reported at pedantic severity with an empty extra and the path as the
    /// Location: in lintian 2.144.0 the Check emits it only as a pointer.
    ///
    /// Walks the packaging File tree (Source tree, or Source package Patched
    /// files). Quilt `.pc/` is filtered out and `debian/missing-sources/` is
    /// skipped. The sibling `source-is-missing` (LN1390) reports those with no
    /// `.py` source.
    SourceContainsPrebuiltPythonObject,
    code = "LN1389",
    tag = "source-contains-prebuilt-python-object",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsPrebuiltPythonObject {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let paths: Vec<_> = visited_files(files)
            .filter(|entry| is_prebuilt_python(files, entry))
            .map(|entry| entry.path().to_path_buf())
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
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    /// Python 3.8 (magic 3413) and 2.7 (62211) code objects.
    const PYTHON38: &[u8] = b"U\x0d\r\n\0\0\0\0rest";
    const PYTHON27: &[u8] = b"\x03\xf3\r\n\0\0\0\0rest";

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("fake.py", TestFile::new("import fake\n")),
                // A name alone does not make byte code.
                ("fake.pyc", TestFile::new("not byte code")),
                // A magic number no CPython release used.
                (
                    "odd.pyc",
                    TestFile::from_bytes(b"\x01\x00\r\n\0\0\0\0".to_vec()),
                ),
                // The magic number without the line ending.
                ("short.pyc", TestFile::from_bytes(b"U\x0d".to_vec())),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (".pc/fake.pyc", TestFile::from_bytes(PYTHON38.to_vec())),
                (
                    "debian/missing-sources/fake.pyc",
                    TestFile::from_bytes(PYTHON38.to_vec()),
                ),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "__pycache__/fake.cpython-38.pyc",
                    TestFile::from_bytes(PYTHON38.to_vec()),
                ),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("fake.pyc", TestFile::from_bytes(PYTHON27.to_vec())),
                ("fake.pyo", TestFile::from_bytes(PYTHON27.to_vec())),
                // The name is not what makes it byte code.
                ("module", TestFile::from_bytes(PYTHON38.to_vec())),
            ],
            // A hardlink is a file too.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "fake.pyc",
                    TestFile::from_bytes(PYTHON38.to_vec()).hardlink(),
                ),
            ],
        ];
        SourceTreeTester::new(SourceContainsPrebuiltPythonObject, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::from(CONTROL),
        )])];
        let fail = vec![SourcePackageCase::from(vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("fake.pyc", TestFile::from_bytes(PYTHON38.to_vec())),
        ])];
        SourcePackageTester::new(SourceContainsPrebuiltPythonObject, pass, fail)
            .test_and_snapshot();
    }
}
