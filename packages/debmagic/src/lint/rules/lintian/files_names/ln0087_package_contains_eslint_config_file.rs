use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct PackageContainsEslintConfigFile;

declare_rule! {
    /// An installed path's final component starts with `.eslintrc`.
    ///
    /// `.eslintrc.js` counts. Udebs are checked too. Extra is empty; the path is
    /// the Location.
    PackageContainsEslintConfigFile,
    code = "LN0087",
    tag = "package-contains-eslint-config-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsEslintConfigFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::Eslint, Severity::Error);
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
                ("usr/share/doc/pkg/eslintrc".to_string(), TestFile::new("x")),
                (
                    "usr/share/doc/pkg/foo.eslintrc".to_string(),
                    TestFile::new("x"),
                ),
            ]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![
                (
                    "usr/share/doc/pkg/.eslintrc".to_string(),
                    TestFile::new("x"),
                ),
                (
                    "usr/share/doc/pkg/.eslintrc.js".to_string(),
                    TestFile::new("x"),
                ),
            ]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/pkg/.eslintrc".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsEslintConfigFile, pass, fail).test_and_snapshot();
    }
}
