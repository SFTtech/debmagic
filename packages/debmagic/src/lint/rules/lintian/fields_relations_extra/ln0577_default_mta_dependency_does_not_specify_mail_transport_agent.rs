use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use crate::lint::package_relation::PackageRelation;

use super::super::fields_relations_more::{BINARY_DEP_FIELDS, SOURCE_RELATION_FIELDS};

pub struct DefaultMtaDependencyDoesNotSpecifyMailTransportAgent;

declare_rule! {
    /// A relation names `default-mta` and not `mail-transport-agent`.
    ///
    /// Any alternative counts, including one after `|`, and an `:arch`
    /// qualifier is ignored. Binary fields are `Depends`, `Pre-Depends`,
    /// `Recommends`, and `Suggests`. Source fields are the build-dependency
    /// and build-conflict fields. Extra is the field and its unfolded value.
    /// Applies to debs, udebs, the Source tree, and the Source package.
    DefaultMtaDependencyDoesNotSpecifyMailTransportAgent,
    code = "LN0577",
    tag = "default-mta-dependency-does-not-specify-mail-transport-agent",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DefaultMtaDependencyDoesNotSpecifyMailTransportAgent {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx
            .debian_control()
            .map(|control| messages(control.source(), SOURCE_RELATION_FIELDS, true))
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

impl BinaryPackageRule for DefaultMtaDependencyDoesNotSpecifyMailTransportAgent {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = messages(
            ctx.installation_control().fields(),
            BINARY_DEP_FIELDS,
            false,
        );
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

/// The field as lintian reads it. Lintian reads source relations from the
/// `.dsc`, where `dpkg-source` has dropped every dependency another one
/// already implies; `simplify` does the same here (the first of equal
/// dependencies stays).
fn effective(
    fields: &ControlParagraph,
    field: &str,
    simplify: bool,
) -> Option<(PackageRelation, String)> {
    let relation = fields.relation(field)?;
    if !simplify {
        return Some((relation, fields.unfolded_value(field)?));
    }
    let texts: Vec<String> = relation
        .dependencies()
        .iter()
        .map(|alternatives| {
            alternatives
                .iter()
                .map(|alternative| alternative.text())
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .collect();
    let kept: Vec<&String> = texts
        .iter()
        .enumerate()
        .filter(|(index, text)| {
            !texts.iter().enumerate().any(|(other, other_text)| {
                other != *index
                    && PackageRelation::parse(other_text).satisfies(text)
                    && (!PackageRelation::parse(text).satisfies(other_text) || other < *index)
            })
        })
        .map(|(_, text)| text)
        .collect();
    let value = kept
        .iter()
        .map(|text| text.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Some((PackageRelation::parse(&value), value))
}

fn messages(fields: &ControlParagraph, names: &[&str], simplify: bool) -> Vec<String> {
    names
        .iter()
        .filter_map(|field| {
            let (relation, value) = effective(fields, field, simplify)?;
            let has_default = relation
                .alternatives()
                .any(|alternative| alternative.package() == "default-mta");
            let has_agent = relation
                .alternatives()
                .any(|alternative| alternative.package() == "mail-transport-agent");
            if !has_default || has_agent {
                return None;
            }
            Some(format!("{field}: {value}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageTester, SourceTreeTester, TestFile};

    use super::super::testing::{binary, control};
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(&[("Build-Depends", "debhelper")]),
            control(&[("Build-Depends", "default-mta | mail-transport-agent")]),
            control(&[("Build-Depends", "default-mta, mail-transport-agent:any")]),
        ];
        let fail = vec![
            control(&[("Build-Depends", "debhelper-compat (= 14), default-mta")]),
            // dpkg-source drops the implied clause, so only `default-mta` remains.
            control(&[(
                "Build-Depends",
                "debhelper-compat (= 14),\n default-mta | mail-transport-agent,\n default-mta",
            )]),
            control(&[
                ("Build-Depends-Indep", "exim4 | default-mta"),
                ("Build-Conflicts", "default-mta"),
            ]),
        ];
        SourceTreeTester::new(
            DefaultMtaDependencyDoesNotSpecifyMailTransportAgent,
            pass,
            fail,
        )
        .test_and_snapshot();
    }

    #[test]
    fn test_binary() {
        let pass = vec![
            binary(&[]),
            binary(&[("Depends", "default-mta | mail-transport-agent")]),
            binary(&[("Depends", "mail-transport-agent")]),
            binary(&[("Conflicts", "default-mta")]),
        ];
        let fail = vec![
            binary(&[("Depends", "default-mta, exim4, nullmailer")]),
            binary(&[
                ("Pre-Depends", "exim4 | default-mta:any"),
                ("Suggests", "default-mta"),
            ]),
            binary(&[("Recommends", "default-mta")]).udeb(),
        ];
        BinaryPackageTester::new(
            DefaultMtaDependencyDoesNotSpecifyMailTransportAgent,
            pass,
            fail,
        )
        .test_and_snapshot_with_suffix("binary");
    }
}
