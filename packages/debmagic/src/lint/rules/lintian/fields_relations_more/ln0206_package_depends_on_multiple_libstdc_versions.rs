use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::BINARY_DEP_FIELDS;

pub struct PackageDependsOnMultipleLibstdcVersions;

declare_rule! {
    /// One relationship field depends on more than one libstdc++ SONAME.
    ///
    /// The first alternative of each dependency in `Depends`, `Pre-Depends`,
    /// `Recommends`, and `Suggests`, counted separately per field. Later
    /// alternatives and an `:arch` qualifier are ignored. Extra is those
    /// package names. Applies to debs and udebs.
    PackageDependsOnMultipleLibstdcVersions,
    code = "LN0206",
    tag = "package-depends-on-multiple-libstdc-versions",
    default_selected = true,
    experimental = false,
}

/// Packages lintian still treats as distinct libstdc++ SONAMEs.
const LIBSTDC: &[&str] = &[
    "libstdc++2.9-glibc2.1",
    "libstdc++2.10",
    "libstdc++2.10-glibc2.2",
    "libstdc++3",
    "libstdc++3.0",
    "libstdc++4",
    "libstdc++5",
    "libstdc++6",
    "lib64stdc++6",
];

impl BinaryPackageRule for PackageDependsOnMultipleLibstdcVersions {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = {
            let fields = ctx.installation_control().fields();
            BINARY_DEP_FIELDS
                .iter()
                .filter_map(|field| {
                    let relation = fields.relation(field)?;
                    let seen: Vec<&str> = relation
                        .dependencies()
                        .iter()
                        .filter_map(|dependency| {
                            let package = dependency.first()?.package();
                            LIBSTDC.contains(&package).then_some(package)
                        })
                        .collect();
                    (seen.len() > 1).then(|| seen.join(" "))
                })
                .collect::<Vec<_>>()
        };
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
            fields(&[("Depends", "libstdc++6")]),
            fields(&[("Depends", "libstdc++7, libstdc++7")]),
            fields(&[("Depends", "libc6 | libstdc++6, libstdc++5")]),
            fields(&[("Depends", "libstdc++6 | libstdc++5")]),
            fields(&[("Conflicts", "libstdc++6, libstdc++5")]),
            fields(&[("Depends", "libstdc++6"), ("Pre-Depends", "libstdc++5")]),
        ];
        let fail = vec![
            fields(&[(
                "Depends",
                "libstdc++2.10, libstdc++2.10-glibc2.2, libstdc++3.0",
            )]),
            fields(&[("Depends", "libstdc++6:any, libstdc++5")]),
            fields(&[
                ("Depends", "libstdc++3, libstdc++2.10"),
                ("Recommends", "libstdc++6, libstdc++4"),
            ]),
            fields(&[("Depends", "lib64stdc++6, libstdc++6")]).udeb(),
        ];
        BinaryPackageTester::new(PackageDependsOnMultipleLibstdcVersions, pass, fail)
            .test_and_snapshot();
    }
}
