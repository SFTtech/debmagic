use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageRelationWithSelf;

declare_rule! {
    /// A relation field of the Binary package names the package itself.
    ///
    /// Reads `Depends`, `Pre-Depends`, `Recommends`, `Suggests`, `Enhances`,
    /// and `Breaks` of the Installation control, in that order (`Conflicts`,
    /// `Replaces`, and `Provides` may name the package). Every alternative
    /// whose package name is the package's own, with no `:arch` qualifier,
    /// counts, whatever its version. Extra is the field name, a colon, and the
    /// alternative as written. Applies to debs and udebs.
    PackageRelationWithSelf,
    code = "LN0027",
    tag = "package-relation-with-self",
    default_selected = true,
    experimental = false,
}

const FIELDS: &[&str] = &[
    "Depends",
    "Pre-Depends",
    "Recommends",
    "Suggests",
    "Enhances",
    "Breaks",
];

impl BinaryPackageRule for PackageRelationWithSelf {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let messages: Vec<String> = {
            let fields = ctx.installation_control().fields();
            FIELDS
                .iter()
                .filter_map(|field| Some((*field, fields.relation(field)?)))
                .flat_map(|(field, relation)| {
                    relation
                        .alternatives()
                        .filter(|alternative| {
                            alternative.package() == package
                                && alternative.architecture_qualifier().is_empty()
                        })
                        .map(|alternative| format!("{field}: {}", alternative.text()))
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

    fn fields(fields: &[(&str, &str)]) -> BinaryPackageCase {
        fields.iter().fold(
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            |case, (name, value)| case.field(*name, *value),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            fields(&[]),
            fields(&[("Depends", "other, libexample (>= 1)")]),
            // Conflicts, Replaces, and Provides may name the package.
            fields(&[("Conflicts", "example"), ("Replaces", "example (<< 1)")]),
            fields(&[("Provides", "example")]),
            // An architecture qualifier makes it another package.
            fields(&[("Depends", "example:any")]),
            fields(&[("Depends", "example:native (>= 1)")]),
            // A prefix or suffix is another name.
            fields(&[("Depends", "example-data, example2")]),
            fields(&[("Build-Depends", "example")]),
        ];
        let fail = vec![
            fields(&[("Depends", "example")]),
            fields(&[("Depends", "other, example (<< 3)")]),
            fields(&[("Depends", "other | example (>= 1)")]),
            fields(&[("Breaks", "example (<< 0.1)")]),
            fields(&[("Pre-Depends", "example"), ("Recommends", "example")]),
            fields(&[("Suggests", "example"), ("Enhances", "example")]),
            fields(&[("Depends", "example(<< 3)")]),
            fields(&[("Depends", "a,\n example")]),
            fields(&[("Depends", "example [amd64]")]),
            fields(&[("Depends", "example"), ("Breaks", "example (<< 1)")]),
            fields(&[("Depends", "example")]).udeb(),
        ];
        BinaryPackageTester::new(PackageRelationWithSelf, pass, fail).test_and_snapshot();
    }
}
