use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::BinaryPackageRule;

pub struct PythonPackageMissingDependsOnPython;

declare_rule! {
    /// A Python 3 module is installed without a dependency on Python 3.
    ///
    /// A regular file or hardlink under `usr/lib/python3` or `usr/lib/python3.Y`,
    /// in `site-packages` or `dist-packages`. Packages whose names end in
    /// `-dbg` are exempt. The package depends on Python 3 when `Pre-Depends`,
    /// `Depends`, `Recommends`, and `Suggests`, together with `Provides` and
    /// the package's own name, imply `python3-minimal:any | python3:any`.
    /// A Python 2 path is not this tag: lintian has no required dependency for
    /// it, and satisfying an empty condition is true. One finding, with an
    /// empty extra and no location. Udebs are checked too.
    PythonPackageMissingDependsOnPython,
    code = "LN0452",
    tag = "python-package-missing-depends-on-python",
    default_selected = true,
    experimental = false,
}

/// lintian's `%REQUIRED_DEPENDS` for `python3`.
const PYTHON3_DEPENDS: &str = "python3-minimal:any | python3:any";

static PYTHON_MODULES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib/(python[23])[\d.]*/(?:site|dist)-packages").expect("valid regex")
});

impl BinaryPackageRule for PythonPackageMissingDependsOnPython {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name().ends_with("-dbg") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let has_python3 = files.entries().iter().any(|entry| {
            matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                && entry.path().to_str().is_some_and(|path| {
                    PYTHON_MODULES.captures(path).is_some_and(|captures| {
                        captures
                            .get(1)
                            .is_some_and(|version| version.as_str() == "python3")
                    })
                })
        });
        if !has_python3 {
            return;
        }
        let fields = ctx.installation_control().fields();
        if !depends_on_python3(fields, ctx.package_name()) {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

/// `relation('all')` AND `Provides` AND the package name imply Python 3.
fn depends_on_python3(fields: &ControlParagraph, package_name: &str) -> bool {
    fields
        .relations_and(&["Pre-Depends", "Depends", "Recommends", "Suggests"])
        .and(&fields.relation("Provides").unwrap_or_default())
        .and(&PackageRelation::parse(package_name))
        .satisfies(PYTHON3_DEPENDS)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn module(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("x"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            module("usr/bin/lintian"),
            module("usr/lib/python3/dist-packages/lintian.py").field("Depends", "python3"),
            module("usr/lib/python3/dist-packages/lintian.py").field("Depends", "python3:any"),
            module("usr/lib/python3/dist-packages/lintian.py").field("Depends", "python3-minimal"),
            module("usr/lib/python3/dist-packages/lintian.py").field("Recommends", "python3"),
            module("usr/lib/python3/dist-packages/lintian.py")
                .field("Suggests", "python3-minimal:any"),
            module("usr/lib/python3/dist-packages/lintian.py").field("Provides", "python3"),
            module("usr/lib/python3/dist-packages/lintian.py")
                .field("Provides", "python3 (= 3.11)"),
            module("usr/lib/python3/dist-packages/lintian.py")
                .field("Depends", "python3 | python3-minimal"),
            module("usr/lib/python3/dist-packages/lintian.py").package("python3"),
            module("usr/lib/python3/dist-packages/lintian.py").package("python3-minimal"),
            module("usr/lib/python3/dist-packages/lintian.py").package("python3-foo-dbg"),
            module("usr/lib/python2.7/dist-packages/lintian.py"),
            module("usr/lib/python3/lintian.py"),
            BinaryPackageCase::from(vec![(
                "usr/lib/python3/dist-packages".to_string(),
                TestFile::directory(),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/python3/dist-packages/lintian.py".to_string(),
                TestFile::symlink("elsewhere.py"),
            )]),
        ];
        let fail = vec![
            module("usr/lib/python3/dist-packages/lintian.py"),
            module("usr/lib/python3.11/site-packages/lintian.py"),
            BinaryPackageCase::from(vec![(
                "usr/lib/python3/dist-packages/lintian.py".to_string(),
                TestFile::new("x").hardlink(),
            )]),
            module("usr/lib/python3/site-packages/lintian.py").udeb(),
            module("usr/lib/python3/dist-packages/lintian.py").field("Depends", "python3 | foo"),
            module("usr/lib/python3/dist-packages/lintian.py").field("Depends", "python3 [amd64]"),
            module("usr/lib/python3/dist-packages/lintian.py").field("Depends", "python3:amd64"),
            module("usr/lib/python3/dist-packages/lintian.py")
                .field("Depends", "python3 <!nocheck>"),
            module("usr/lib/python3/dist-packages/lintian.py").field("Breaks", "python3"),
            BinaryPackageCase::from(vec![
                (
                    "usr/lib/python2.7/dist-packages/old.py".to_string(),
                    TestFile::new("x"),
                ),
                (
                    "usr/lib/python3/dist-packages/new.py".to_string(),
                    TestFile::new("x"),
                ),
            ]),
        ];
        BinaryPackageTester::new(PythonPackageMissingDependsOnPython, pass, fail)
            .test_and_snapshot();
    }
}
