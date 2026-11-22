use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::data;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

/// Relationship fields lintian's obsolete-package tags read.
const RELATION_FIELDS: &[&str] = &["Depends", "Pre-Depends", "Recommends", "Suggests"];

/// `data/fields/obsolete-packages`: `name` or `name => replacement`.
static OBSOLETE: LazyLock<Vec<(String, String)>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/fields/obsolete-packages"))
        .map(|line| {
            let (name, replacement) = line
                .split_once("=>")
                .map(|(name, replacement)| (name.trim(), replacement.trim()))
                .unwrap_or((line, ""));
            (name.to_string(), replacement.to_string())
        })
        .collect()
});

fn replacement(package: &str) -> Option<&'static str> {
    OBSOLETE
        .iter()
        .find(|(name, _)| name == package)
        .map(|(_, replacement)| replacement.as_str())
}

pub struct OredDependsOnObsoletePackage;

declare_rule! {
    /// An obsolete package is one alternative of an ORed group whose first
    /// alternative is current, such as `foo | obsolete-pkg`.
    ///
    /// `Depends`, `Pre-Depends`, `Recommends`, and `Suggests` of the
    /// Installation control. An obsolete alternative is reported unless it is
    /// the first alternative of its dependency or every alternative is
    /// obsolete (those are `depends-on-obsolete-package`). Extra is the field,
    /// the alternative as written, and ` => replacement` when the data file
    /// names one. Applies to debs and udebs.
    OredDependsOnObsoletePackage,
    code = "LN1367",
    tag = "ored-depends-on-obsolete-package",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for OredDependsOnObsoletePackage {
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
                                .filter(|alternative| !(all || alternative.package() == first))
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
            ctx.diagnostic(Diagnostic::warn(message).with_severity(Severity::Info));
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
            depends("foo | bar"),
            depends("xbase-clients"),
        ];
        let fail = vec![
            depends("foo | xbase-clients (>= 0.1)"),
            depends("foo | xbase-clients (>= 0.1)").udeb(),
        ];
        BinaryPackageTester::new(OredDependsOnObsoletePackage, pass, fail).test_and_snapshot();
    }
}
