use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct PackageContainsIconCacheInGenericDir;

declare_rule! {
    /// The package ships `usr/share/icons/hicolor/icon-theme.cache`.
    ///
    /// Only that exact path. Udebs are checked too. Extra is empty; the path is
    /// the Location.
    PackageContainsIconCacheInGenericDir,
    code = "LN0082",
    tag = "package-contains-icon-cache-in-generic-dir",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsIconCacheInGenericDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::IconCache, Severity::Warning);
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
                    "usr/share/icons/hicolor/index.theme".to_string(),
                    TestFile::new("x"),
                ),
                (
                    "usr/share/icons/gnome/icon-theme.cache".to_string(),
                    TestFile::new("x"),
                ),
            ]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/icons/hicolor/icon-theme.cache".to_string(),
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/icons/hicolor/icon-theme.cache".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsIconCacheInGenericDir, pass, fail)
            .test_and_snapshot();
    }
}
