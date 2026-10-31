use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct PackageContainsPythonDoctreeFile;

declare_rule! {
    /// An installed path's final component ends in `.doctree` or `.doctree.gz`.
    ///
    /// `.doctree.bak` is not this Tag. Udebs are checked too. Extra is empty; the
    /// path is the Location.
    PackageContainsPythonDoctreeFile,
    code = "LN0084",
    tag = "package-contains-python-doctree-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsPythonDoctreeFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::PythonDoctree, Severity::Warning);
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
                    "usr/share/foo/foo.doctree.bak".to_string(),
                    TestFile::new("x"),
                ),
                ("usr/share/foo/doctree".to_string(), TestFile::new("x")),
            ]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![
                ("usr/share/foo/foo.doctree".to_string(), TestFile::new("x")),
                (
                    "usr/share/foo/foo.doctree.gz".to_string(),
                    TestFile::new("x"),
                ),
            ]),
            BinaryPackageCase::from(vec![(
                "usr/share/foo/foo.doctree".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsPythonDoctreeFile, pass, fail).test_and_snapshot();
    }
}
