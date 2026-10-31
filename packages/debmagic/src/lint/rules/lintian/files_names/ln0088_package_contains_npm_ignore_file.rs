use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct PackageContainsNpmIgnoreFile;

declare_rule! {
    /// An installed path's final component is `.npmignore`.
    ///
    /// Udebs are checked too. Extra is empty; the path is the Location.
    PackageContainsNpmIgnoreFile,
    code = "LN0088",
    tag = "package-contains-npm-ignore-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsNpmIgnoreFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::NpmIgnore, Severity::Error);
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
                    "usr/share/doc/pkg/.npmignore.bak".to_string(),
                    TestFile::new("x"),
                ),
                (
                    "usr/share/doc/pkg/npmignore".to_string(),
                    TestFile::new("x"),
                ),
            ]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/doc/pkg/.npmignore".to_string(),
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/pkg/.npmignore".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsNpmIgnoreFile, pass, fail).test_and_snapshot();
    }
}
