use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::obsolete::resolves_to_file;

pub struct DebianPycompatIsObsolete;

declare_rule! {
    /// The patched tree has a `debian/pycompat` file.
    ///
    /// No modern Python helper reads it. `debian/pycompat` is followed through
    /// symlinks inside the tree, and a hardlink counts as a file. The file is
    /// the Location; the extra is empty. Severity is info and the Tag is not
    /// selected by default, as in lintian.
    DebianPycompatIsObsolete,
    code = "LN1571",
    tag = "debian-pycompat-is-obsolete",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DebianPycompatIsObsolete {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        if resolves_to_file(files, "debian/pycompat") {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_location(Location {
                        path: "debian/pycompat".into(),
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
            TestCase::source_tree(vec![("debian/pyversions", TestFile::new("3.0\n"))]),
            TestCase::source_tree(vec![("debian/pycompat", TestFile::directory())]),
            // A symlink that leaves the tree, or dangles, is not a file.
            TestCase::source_tree(vec![(
                "debian/pycompat",
                TestFile::symlink("/etc/pycompat"),
            )]),
            TestCase::source_tree(vec![("debian/pycompat", TestFile::symlink("missing"))]),
        ];
        let fail = vec![
            TestCase::source_tree(vec![("debian/pycompat", TestFile::new("2\n"))]),
            TestCase::source_tree(vec![
                ("debian/pycompat.real", TestFile::new("2\n")),
                ("debian/pycompat", TestFile::symlink("pycompat.real")),
            ]),
            TestCase::source_tree(vec![
                ("debian/real", TestFile::new("2\n")),
                ("debian/pycompat", TestFile::symlink("../debian/real")),
            ]),
            TestCase::source_tree(vec![("debian/pycompat", TestFile::new("2\n").hardlink())]),
        ];
        SourceTreeTester::new(DebianPycompatIsObsolete, pass, fail).test_and_snapshot();
    }
}
