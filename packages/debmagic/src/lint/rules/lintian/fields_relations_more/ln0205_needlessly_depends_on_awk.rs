use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::BINARY_DEP_FIELDS;

pub struct NeedlesslyDependsOnAwk;

declare_rule! {
    /// A dependency alternative is the virtual package `awk` and gives no version.
    ///
    /// `Depends`, `Pre-Depends`, `Recommends`, and `Suggests`. `base-files` may
    /// depend on `awk`. A version constraint suppresses the Tag; there is no
    /// version comparison. Extra is the field name. Applies to debs and udebs.
    NeedlesslyDependsOnAwk,
    code = "LN0205",
    tag = "needlessly-depends-on-awk",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NeedlesslyDependsOnAwk {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let messages = {
            let fields = ctx.installation_control().fields();
            if package == "base-files" {
                Vec::new()
            } else {
                BINARY_DEP_FIELDS
                    .iter()
                    .filter_map(|field| Some((*field, fields.relation(field)?)))
                    .flat_map(|(field, relation)| {
                        relation
                            .alternatives()
                            .filter(|alternative| {
                                alternative.package() == "awk" && alternative.version().is_none()
                            })
                            .map(|_| field.to_string())
                            .collect::<Vec<_>>()
                    })
                    .collect()
            }
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
            fields(&[("Depends", "gawk")]),
            fields(&[("Depends", "mawk")]),
            fields(&[("Depends", "awk (>= 1)")]),
            fields(&[("Conflicts", "awk")]),
            fields(&[("Depends", "awk"), ("Pre-Depends", "awk|gawk")]).package("base-files"),
        ];
        let fail = vec![
            fields(&[("Depends", "awk")]),
            fields(&[("Pre-Depends", "awk|gawk")]),
            fields(&[("Depends", "gawk | awk"), ("Suggests", "awk")]),
            fields(&[("Depends", "awk")]).udeb(),
        ];
        BinaryPackageTester::new(NeedlesslyDependsOnAwk, pass, fail).test_and_snapshot();
    }
}
