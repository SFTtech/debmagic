use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct PackageContainsPythonHeaderInIncorrectDirectory;

declare_rule! {
    /// A `.h` file sits directly in `usr/include/python3.0/` through `python3.7/`.
    ///
    /// `python3.8` and later are not flagged, nor is a header nested in a
    /// subdirectory of those include directories. Udebs are checked too. Extra is
    /// empty; the path is the Location.
    PackageContainsPythonHeaderInIncorrectDirectory,
    code = "LN0085",
    tag = "package-contains-python-header-in-incorrect-directory",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsPythonHeaderInIncorrectDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::PythonHeader, Severity::Error);
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
                    "usr/include/python3.8/bad.h".to_string(),
                    TestFile::new("x"),
                ),
                (
                    "usr/include/python3.7/sub/bad.h".to_string(),
                    TestFile::new("x"),
                ),
                (
                    "usr/include/python3.7/bad.hpp".to_string(),
                    TestFile::new("x"),
                ),
            ]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![
                (
                    "usr/include/python3.7/bad.h".to_string(),
                    TestFile::new("x"),
                ),
                ("usr/include/python3.0/x.h".to_string(), TestFile::new("x")),
            ]),
            BinaryPackageCase::from(vec![(
                "usr/include/python3.7/bad.h".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsPythonHeaderInIncorrectDirectory, pass, fail)
            .test_and_snapshot();
    }
}
