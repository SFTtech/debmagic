use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::repeated_library_versions;

pub struct PackageDependsOnMultipleTclVersions;

declare_rule! {
    /// One relationship field depends on more than one Tcl version.
    ///
    /// The first alternative of each dependency in `Depends`, `Pre-Depends`,
    /// `Recommends`, and `Suggests`, counted separately per field. Later
    /// alternatives and an `:arch` qualifier are ignored. `tclx*` is a
    /// different Tag. Extra is those package names. Applies to debs and udebs.
    PackageDependsOnMultipleTclVersions,
    code = "LN0264",
    tag = "package-depends-on-multiple-tcl-versions",
    default_selected = true,
    experimental = false,
}

const TCL: &[&str] = &["tcl74", "tcl8.0", "tcl8.2", "tcl8.3", "tcl8.4", "tcl8.5"];

impl BinaryPackageRule for PackageDependsOnMultipleTclVersions {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = repeated_library_versions(ctx.installation_control().fields(), TCL);
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn fields(items: &[(&str, &str)]) -> BinaryPackageCase {
        items.iter().fold(
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            |case, (name, value)| case.field(*name, *value),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            fields(&[]),
            fields(&[("Depends", "tcl8.6")]),
            fields(&[("Depends", "tcl8.6, tcl8.6")]),
            fields(&[("Depends", "libc6 | tcl8.0, tcl8.2")]),
            fields(&[("Depends", "tcl8.0 | tcl8.2")]),
            fields(&[("Depends", "tclx76, tclx8.2")]),
            fields(&[("Conflicts", "tcl8.0, tcl8.2")]),
            fields(&[("Depends", "tcl8.0"), ("Pre-Depends", "tcl8.2")]),
        ];
        let fail = vec![
            fields(&[("Depends", "tcl74, tcl8.0, tcl8.2")]),
            fields(&[("Depends", "tcl8.0:any, tcl8.2")]),
            fields(&[
                ("Depends", "tcl8.0, tcl8.2"),
                ("Recommends", "tcl8.4, tcl8.5"),
            ]),
            fields(&[("Depends", "tcl8.3, tcl8.4")]).udeb(),
        ];
        BinaryPackageTester::new(PackageDependsOnMultipleTclVersions, pass, fail)
            .test_and_snapshot();
    }
}
