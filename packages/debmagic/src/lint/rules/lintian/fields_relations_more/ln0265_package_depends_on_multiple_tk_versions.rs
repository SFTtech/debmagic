use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::repeated_library_versions;

pub struct PackageDependsOnMultipleTkVersions;

declare_rule! {
    /// One relationship field depends on more than one Tk version.
    ///
    /// The first alternative of each dependency in `Depends`, `Pre-Depends`,
    /// `Recommends`, and `Suggests`, counted separately per field. Later
    /// alternatives and an `:arch` qualifier are ignored. Extra is those
    /// package names. Applies to debs and udebs.
    PackageDependsOnMultipleTkVersions,
    code = "LN0265",
    tag = "package-depends-on-multiple-tk-versions",
    default_selected = true,
    experimental = false,
}

const TK: &[&str] = &["tk40", "tk8.0", "tk8.2", "tk8.3", "tk8.4", "tk8.5"];

impl BinaryPackageRule for PackageDependsOnMultipleTkVersions {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = repeated_library_versions(ctx.installation_control().fields(), TK);
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
            fields(&[("Depends", "tk8.6")]),
            fields(&[("Depends", "tk8.6, tk8.6")]),
            fields(&[("Depends", "libc6 | tk8.2, tk8.3")]),
            fields(&[("Depends", "tk8.2 | tk8.3")]),
            fields(&[("Conflicts", "tk8.2, tk8.3")]),
            fields(&[("Depends", "tk8.2"), ("Pre-Depends", "tk8.3")]),
        ];
        let fail = vec![
            fields(&[("Depends", "tk40, tk8.0, tk8.2")]),
            fields(&[("Depends", "tk8.2:any, tk8.3")]),
            fields(&[("Depends", "tk8.2, tk8.3"), ("Suggests", "tk8.4, tk8.5")]),
            fields(&[("Depends", "tk8.4, tk8.5")]).udeb(),
        ];
        BinaryPackageTester::new(PackageDependsOnMultipleTkVersions, pass, fail)
            .test_and_snapshot();
    }
}
