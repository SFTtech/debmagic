use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct PackageContainsFileInUsrShareHal;

declare_rule! {
    /// A path sits under `usr/share/hal/`.
    ///
    /// The `usr/share/hal` directory itself is not flagged. Udebs are checked too.
    /// Extra is empty; the path is the Location.
    PackageContainsFileInUsrShareHal,
    code = "LN0081",
    tag = "package-contains-file-in-usr-share-hal",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsFileInUsrShareHal {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::Hal, Severity::Warning);
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
                ("usr/share/hal".to_string(), TestFile::directory()),
                ("usr/share/doc/hal/foo".to_string(), TestFile::new("x")),
            ]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/hal/foo.fdi".to_string(),
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/hal/foo.fdi".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsFileInUsrShareHal, pass, fail).test_and_snapshot();
    }
}
