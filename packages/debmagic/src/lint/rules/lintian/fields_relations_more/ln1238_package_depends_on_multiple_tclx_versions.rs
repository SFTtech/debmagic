use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageDependsOnMultipleTclxVersions;

declare_rule! {
    /// One of `Depends`, `Pre-Depends`, `Recommends` or `Suggests` names more
    /// than one known tclx version as the first alternative of its
    /// comma-separated relations. Reported once per such field; Extra lists
    /// the matching package names separated by spaces, in field order.
    /// Reads the Installation control. Udebs are checked too.
    PackageDependsOnMultipleTclxVersions,
    code = "LN1238",
    tag = "package-depends-on-multiple-tclx-versions",
    default_selected = true,
    experimental = false,
}

/// The tclx packages lintian knows (`%known_tclxs`).
const KNOWN_TCLXS: &[&str] = &["tclx76", "tclx8.0.4", "tclx8.2", "tclx8.3", "tclx8.4"];

const DEPENDENCY_FIELDS: &[&str] = &["Depends", "Pre-Depends", "Recommends", "Suggests"];

impl BinaryPackageRule for PackageDependsOnMultipleTclxVersions {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let mut findings = Vec::new();
        let fields = ctx.installation_control().fields();
        for field in DEPENDENCY_FIELDS {
            let Some(relation) = fields.relation(field) else {
                continue;
            };
            let seen: Vec<&str> = relation
                .dependencies()
                .iter()
                .filter_map(|alternatives| alternatives.first())
                .map(|first| first.package())
                .filter(|package| KNOWN_TCLXS.contains(package))
                .collect();
            if seen.len() > 1 {
                findings.push(seen.join(" "));
            }
        }
        for finding in findings {
            ctx.diagnostic(Diagnostic::error(finding));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn depends(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Depends", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            depends("tclx8.2"),
            // Only the first alternative of each relation counts.
            depends("tclx8.2 | tclx8.3, libc6"),
            depends("tclx8.2:any, libc6"),
            // Two tclx versions in different fields are not reported.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Depends", "tclx8.2")
                .field("Recommends", "tclx8.3"),
            // Conflicts is not one of the checked fields.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Conflicts", "tclx8.2, tclx8.3"),
        ];
        let fail = vec![
            depends("tclx76, tclx8.0.4, tclx8.2"),
            depends("tclx8.2, tclx8.2"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Suggests", "tclx8.3, tclx8.4 | libc6"),
            depends("tclx8.2, tclx8.3").udeb(),
        ];
        BinaryPackageTester::new(PackageDependsOnMultipleTclxVersions, pass, fail)
            .test_and_snapshot();
    }
}
