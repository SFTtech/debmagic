use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{BINARY_DEP_FIELDS, obsolete_bootstrap_messages};

pub struct DependsOnObsoleteBootstrap;

declare_rule! {
    /// A dependency names an obsolete Bootstrap package.
    ///
    /// `Depends`, `Pre-Depends`, `Recommends`, and `Suggests`. The names are
    /// `libjs-bootstrap`, `fonts-glyphicons-halflings`, and `libjs-bootstrap4`,
    /// in any alternative. Conflicts and breaks are not this Tag. A version
    /// constraint does not matter. Extra is the field and the package name.
    /// Applies to debs and udebs.
    DependsOnObsoleteBootstrap,
    code = "LN0269",
    tag = "depends-on-obsolete-bootstrap",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DependsOnObsoleteBootstrap {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages =
            obsolete_bootstrap_messages(ctx.installation_control().fields(), BINARY_DEP_FIELDS);
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
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
            fields(&[("Depends", "libjs-bootstrap5")]),
            fields(&[("Conflicts", "libjs-bootstrap")]),
            fields(&[("Build-Depends", "libjs-bootstrap")]),
        ];
        let fail = vec![
            fields(&[
                ("Depends", "libjs-bootstrap, fonts-glyphicons-halflings"),
                ("Suggests", "libjs-bootstrap4 (>= 4)"),
            ]),
            fields(&[("Pre-Depends", "foo | libjs-bootstrap")]),
            fields(&[("Recommends", "libjs-bootstrap:any")]).udeb(),
        ];
        BinaryPackageTester::new(DependsOnObsoleteBootstrap, pass, fail).test_and_snapshot();
    }
}
