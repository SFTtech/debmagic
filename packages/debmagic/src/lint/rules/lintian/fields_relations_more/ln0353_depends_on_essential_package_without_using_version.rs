use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::essential::is_essential;

pub struct DependsOnEssentialPackageWithoutUsingVersion;

declare_rule! {
    /// `Depends` or `Pre-Depends` names an essential package and gives no version.
    ///
    /// Installation control. Essential packages are lintian's `fields/essential`
    /// list. `diffutils` and `dash` are excluded. Presence of a `(op version)`
    /// constraint is enough; versions are not compared. `Recommends` and
    /// `Suggests` are not this Tag. Extra is the field and the alternative as
    /// written. Udebs are checked too.
    DependsOnEssentialPackageWithoutUsingVersion,
    code = "LN0353",
    tag = "depends-on-essential-package-without-using-version",
    default_selected = true,
    experimental = false,
}

const DEP_FIELDS: &[&str] = &["Depends", "Pre-Depends"];
const EXCLUDED: &[&str] = &["diffutils", "dash"];

impl BinaryPackageRule for DependsOnEssentialPackageWithoutUsingVersion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = messages(ctx.installation_control().fields());
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn messages(fields: &ControlParagraph) -> Vec<String> {
    DEP_FIELDS
        .iter()
        .filter_map(|field| Some((*field, fields.relation(field)?)))
        .flat_map(|(field, relation)| {
            relation
                .alternatives()
                .filter(|alternative| {
                    !EXCLUDED.contains(&alternative.package())
                        && alternative.version().is_none()
                        && is_essential(alternative.package())
                })
                .map(|alternative| format!("{field}: {}", alternative.text()))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package(fields: &[(&str, &str)]) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(Vec::<(&str, TestFile)>::new());
        for (name, value) in fields {
            case = case.field(*name, *value);
        }
        case
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            package(&[("Depends", "bash (>= 5), dash, diffutils, libc6")]),
            package(&[("Pre-Depends", "dash")]),
            package(&[("Recommends", "dpkg, bash"), ("Suggests", "coreutils")]),
            package(&[("Depends", "awk | gawk")]),
        ];
        let fail = vec![
            package(&[(
                "Depends",
                "dpkg, bash (>> 2.0), coreutils, foo | tar, diffutils",
            )]),
            package(&[("Pre-Depends", "bash"), ("Depends", "grep [amd64]")]),
            package(&[("Depends", "sed")]).udeb(),
        ];
        BinaryPackageTester::new(DependsOnEssentialPackageWithoutUsingVersion, pass, fail)
            .test_and_snapshot();
    }
}
