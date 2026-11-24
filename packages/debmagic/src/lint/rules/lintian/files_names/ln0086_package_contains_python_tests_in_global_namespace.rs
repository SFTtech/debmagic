use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct PackageContainsPythonTestsInGlobalNamespace;

declare_rule! {
    /// A `test_*.py` module sits directly in `dist-packages` or `site-packages`.
    ///
    /// A test module inside a package directory is not flagged. Udebs are checked
    /// too. Extra is empty; the path is the Location.
    PackageContainsPythonTestsInGlobalNamespace,
    code = "LN0086",
    tag = "package-contains-python-tests-in-global-namespace",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsPythonTestsInGlobalNamespace {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::PythonTests, Severity::Warning);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            BinaryPackageCase::from(vec![
                (
                    "usr/lib/python3/dist-packages/pkg/test_foo.py".to_string(),
                    TestFile::new("x"),
                ),
                (
                    "usr/lib/python3/dist-packages/foo.py".to_string(),
                    TestFile::new("x"),
                ),
            ]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![
                (
                    "usr/lib/python3/dist-packages/test_foo.py".to_string(),
                    TestFile::new("x"),
                ),
                (
                    "usr/lib/python3/site-packages/test_bar.py".to_string(),
                    TestFile::new("x"),
                ),
            ]),
            BinaryPackageCase::from(vec![(
                "usr/lib/python3/dist-packages/test_foo.py".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsPythonTestsInGlobalNamespace, pass, fail)
            .test_and_snapshot();
    }
}
