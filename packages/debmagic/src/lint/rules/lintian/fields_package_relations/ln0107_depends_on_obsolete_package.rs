use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::RELATION_FIELDS;
use super::obsolete_packages::replacement;

pub struct DependsOnObsoletePackage;

declare_rule! {
    /// A dependency alternative names a package lintian lists as obsolete.
    ///
    /// `Depends`, `Pre-Depends`, `Recommends`, and `Suggests` of the
    /// Installation control. The alternative is this Tag when it is the first
    /// of its dependency or every alternative is obsolete; a later obsolete
    /// alternative beside a current one is `ored-depends-on-obsolete-package`.
    /// Extra is the field, the alternative as written, and ` => replacement`
    /// when the data file names one. Applies to debs and udebs.
    DependsOnObsoletePackage,
    code = "LN0107",
    tag = "depends-on-obsolete-package",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DependsOnObsoletePackage {
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
                        .flat_map(|dependency| {
                            let obsolete: Vec<_> = dependency
                                .iter()
                                .filter(|alternative| replacement(alternative.package()).is_some())
                                .collect();
                            let first = dependency
                                .first()
                                .map(|alternative| alternative.package())
                                .unwrap_or("");
                            let all = !dependency.is_empty() && obsolete.len() == dependency.len();
                            obsolete
                                .into_iter()
                                .filter(|alternative| all || alternative.package() == first)
                                .map(|alternative| {
                                    let suffix = replacement(alternative.package())
                                        .filter(|value| !value.is_empty())
                                        .map(|value| format!(" => {value}"))
                                        .unwrap_or_default();
                                    format!("{field}: {}{suffix}", alternative.text())
                                })
                                .collect::<Vec<_>>()
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
            fields(&[("Depends", "libc6")]),
            fields(&[("Depends", "current | gaim")]),
            fields(&[("Conflicts", "gaim")]),
        ];
        let fail = vec![
            fields(&[("Depends", "gaim (>= 0.1)")]),
            fields(&[("Depends", "pkg-config")]),
            fields(&[("Recommends", "gs")]),
            fields(&[("Suggests", "x-dev")]),
            fields(&[("Depends", "gaim | gs")]),
            fields(&[("Depends", "ruby1.8")]).udeb(),
        ];
        BinaryPackageTester::new(DependsOnObsoletePackage, pass, fail).test_and_snapshot();
    }
}
