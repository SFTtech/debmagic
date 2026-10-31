use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::BINARY_DEP_FIELDS;

pub struct DependsOnPythonMinimal;

declare_rule! {
    /// A dependency alternative is `python-minimal` or `pythonN.M-minimal`.
    ///
    /// `Depends`, `Pre-Depends`, `Recommends`, and `Suggests`. The package name
    /// matches `python`, then digits and dots, then `-minimal`. A package
    /// whose own name matches is exempt. Extra is the field name. Applies to
    /// debs and udebs.
    DependsOnPythonMinimal,
    code = "LN0203",
    tag = "depends-on-python-minimal",
    default_selected = true,
    experimental = false,
}

static PYTHON_MINIMAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^python[\d.]*-minimal$").expect("valid regex"));

impl BinaryPackageRule for DependsOnPythonMinimal {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let messages = {
            let fields = ctx.installation_control().fields();
            if PYTHON_MINIMAL.is_match(&package) {
                Vec::new()
            } else {
                BINARY_DEP_FIELDS
                    .iter()
                    .filter_map(|field| Some((*field, fields.relation(field)?)))
                    .flat_map(|(field, relation)| {
                        relation
                            .alternatives()
                            .filter(|alternative| PYTHON_MINIMAL.is_match(alternative.package()))
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
            fields(&[("Depends", "python3")]),
            fields(&[("Depends", "python-minimal-doc")]),
            fields(&[("Conflicts", "python-minimal")]),
            fields(&[("Depends", "python-minimal")]).package("python-minimal"),
            fields(&[("Depends", "python-minimal")]).package("python3-minimal"),
        ];
        let fail = vec![
            fields(&[("Depends", "python-minimal")]),
            fields(&[
                ("Depends", "python-minimal | python3-minimal"),
                ("Suggests", "python2.7-minimal"),
            ]),
            fields(&[("Recommends", "python3.11-minimal")]),
            fields(&[("Depends", "python-minimal")]).udeb(),
        ];
        BinaryPackageTester::new(DependsOnPythonMinimal, pass, fail).test_and_snapshot();
    }
}
