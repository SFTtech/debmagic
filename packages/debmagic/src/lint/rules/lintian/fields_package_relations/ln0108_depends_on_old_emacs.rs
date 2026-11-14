use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::RELATION_FIELDS;
use super::obsolete_packages::obsolete_emacs;

pub struct DependsOnOldEmacs;

declare_rule! {
    /// The first alternative of a dependency is an old Emacs package.
    ///
    /// `Depends`, `Pre-Depends`, `Recommends`, and `Suggests`. Emacs 21, 22,
    /// and 23, with an optional `-el`, `-gtk`, `-nox`, or `-lucid` flavour.
    /// Extra is the field and that package name. Applies to debs and udebs.
    DependsOnOldEmacs,
    code = "LN0108",
    tag = "depends-on-old-emacs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DependsOnOldEmacs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages: Vec<String> = {
            let fields = ctx.installation_control().fields();
            RELATION_FIELDS
                .iter()
                .filter_map(|field| Some((*field, fields.relation(field)?)))
                .flat_map(|(field, relation)| {
                    relation
                        .dependencies()
                        .iter()
                        .filter_map(|dependency| {
                            let package = dependency.first()?.package();
                            obsolete_emacs(package).then(|| format!("{field}: {package}"))
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn dep(field: &str, value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field(field, value)
    }

    #[test]
    fn test() {
        let pass = vec![
            dep("Depends", "emacs"),
            dep("Depends", "emacs24 | emacs23"),
            dep("Conflicts", "emacs21"),
        ];
        let fail = vec![
            dep("Depends", "emacs23"),
            dep("Depends", "emacs23-nox"),
            dep("Depends", "emacs22-gtk"),
            dep("Depends", "emacs21"),
            dep("Recommends", "emacs21-el"),
            dep("Depends", "emacs23-lucid").udeb(),
        ];
        BinaryPackageTester::new(DependsOnOldEmacs, pass, fail).test_and_snapshot();
    }
}
