use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::obsolete::resolves_to_file;

pub struct DebianPyversionsIsObsolete;

declare_rule! {
    /// The patched tree has a `debian/pyversions` file.
    ///
    /// No modern Python helper reads it. `debian/pyversions` is followed through
    /// symlinks inside the tree, and a hardlink counts as a file. The file is
    /// the Location; the extra is empty. Severity is info and the Tag is not
    /// selected by default, as in lintian.
    DebianPyversionsIsObsolete,
    code = "LN1572",
    tag = "debian-pyversions-is-obsolete",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DebianPyversionsIsObsolete {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        if resolves_to_file(files, "debian/pyversions") {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_location(Location {
                        path: "debian/pyversions".into(),
                        line: None,
                        column: None,
                    })
                    .with_severity(Severity::Info),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    #[test]
    fn source() {
        let pass = vec![
            TestCase::source_tree(vec![("README", TestFile::new("x"))]),
            TestCase::source_tree(vec![("debian/pycompat", TestFile::new("3\n"))]),
            TestCase::source_tree(vec![("debian/pyversions", TestFile::directory())]),
            // A symlink that leaves the tree, or dangles, is not a file.
            TestCase::source_tree(vec![(
                "debian/pyversions",
                TestFile::symlink("/etc/pyversions"),
            )]),
            TestCase::source_tree(vec![("debian/pyversions", TestFile::symlink("missing"))]),
        ];
        let fail = vec![
            TestCase::source_tree(vec![("debian/pyversions", TestFile::new("2\n"))]),
            TestCase::source_tree(vec![
                ("debian/pyversions.real", TestFile::new("2\n")),
                ("debian/pyversions", TestFile::symlink("pyversions.real")),
            ]),
            TestCase::source_tree(vec![
                ("debian/real", TestFile::new("2\n")),
                ("debian/pyversions", TestFile::symlink("../debian/real")),
            ]),
            TestCase::source_tree(vec![("debian/pyversions", TestFile::new("2\n").hardlink())]),
        ];
        SourceTreeTester::new(DebianPyversionsIsObsolete, pass, fail).test_and_snapshot();
    }
}
