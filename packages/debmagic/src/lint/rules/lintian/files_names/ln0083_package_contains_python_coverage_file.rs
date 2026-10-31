use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct PackageContainsPythonCoverageFile;

declare_rule! {
    /// An installed path's final component is `.coverage`.
    ///
    /// Any directory. Udebs are checked too. Extra is empty; the path is the
    /// Location.
    PackageContainsPythonCoverageFile,
    code = "LN0083",
    tag = "package-contains-python-coverage-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsPythonCoverageFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::PythonCoverage, Severity::Warning);
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
                ("usr/share/pkg/coverage".to_string(), TestFile::new("x")),
                (
                    "usr/share/pkg/.coverage.bak".to_string(),
                    TestFile::new("x"),
                ),
            ]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/pkg/.coverage".to_string(),
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/pkg/.coverage".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsPythonCoverageFile, pass, fail).test_and_snapshot();
    }
}
