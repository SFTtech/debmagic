use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct PackageContainsFileInEtcSkel;

declare_rule! {
    /// A file under `etc/skel/` is not one of the shell startup files that belong there.
    ///
    /// `.bashrc`, `.bash_logout`, `.kshrc`, `.mkshrc`, and `.profile` are allowed.
    /// The `etc/skel` directory itself is not flagged. Udebs are checked too.
    /// Extra is empty; the path is the Location.
    PackageContainsFileInEtcSkel,
    code = "LN0080",
    tag = "package-contains-file-in-etc-skel",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsFileInEtcSkel {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::EtcSkel, Severity::Error);
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
                ("etc/skel/.bashrc".to_string(), TestFile::new("x")),
                ("etc/skel/.mkshrc".to_string(), TestFile::new("x")),
                ("etc/skel/.profile".to_string(), TestFile::new("x")),
                ("etc/skel".to_string(), TestFile::directory()),
            ]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![
                ("etc/skel/.lintianrc".to_string(), TestFile::new("x")),
                ("etc/skel/subdir/extra".to_string(), TestFile::new("x")),
            ]),
            BinaryPackageCase::from(vec![(
                "etc/skel/.lintianrc".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsFileInEtcSkel, pass, fail).test_and_snapshot();
    }
}
