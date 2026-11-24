use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::super::fields_relations_more::{BINARY_DEP_FIELDS, SOURCE_RELATION_FIELDS};

pub struct DefaultMtaDependencyNotListedFirst;

declare_rule! {
    /// `default-mta` follows another alternative in a dependency.
    ///
    /// A dependency (comma-separated entry) counts when an alternative after
    /// the first one is a package whose name starts with `default-mta`. At
    /// most one Diagnostic per field. Binary fields are `Depends`,
    /// `Pre-Depends`, `Recommends`, and `Suggests`. Source fields are the
    /// build-dependency and build-conflict fields. Extra is the field and its
    /// unfolded value. Applies to debs, udebs, the Source tree, and the
    /// Source package.
    DefaultMtaDependencyNotListedFirst,
    code = "LN0578",
    tag = "default-mta-dependency-not-listed-first",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DefaultMtaDependencyNotListedFirst {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx
            .debian_control()
            .map(|control| messages(control.source(), SOURCE_RELATION_FIELDS))
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

impl BinaryPackageRule for DefaultMtaDependencyNotListedFirst {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = messages(ctx.installation_control().fields(), BINARY_DEP_FIELDS);
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

fn messages(fields: &ControlParagraph, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter_map(|field| {
            let relation = fields.relation(field)?;
            let late = relation.dependencies().iter().any(|alternatives| {
                alternatives
                    .iter()
                    .skip(1)
                    .any(|alternative| alternative.package().starts_with("default-mta"))
            });
            if !late {
                return None;
            }
            let value = fields.unfolded_value(field)?;
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
            control(&[("Build-Depends", "default-mta | exim4")]),
            // Separate dependencies are not alternatives.
            control(&[("Build-Depends", "exim4, default-mta")]),
        ];
        let fail = vec![
            control(&[("Build-Depends", "exim4 | default-mta, debhelper")]),
            control(&[
                ("Build-Depends-Indep", "a, b | c | default-mta:any"),
                ("Build-Conflicts", "x | default-mta-foo"),
            ]),
        ];
        SourceTreeTester::new(DefaultMtaDependencyNotListedFirst, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_binary() {
        let pass = vec![
            binary(&[]),
            binary(&[("Depends", "default-mta | mail-transport-agent")]),
            binary(&[("Conflicts", "exim4 | default-mta")]),
        ];
        let fail = vec![
            binary(&[("Pre-Depends", "exim4 | default-mta | mail-transport-agent")]),
            binary(&[
                ("Depends", "exim4 | default-mta"),
                ("Suggests", "a | b | default-mta"),
            ]),
            binary(&[("Recommends", "a | default-mta (>= 1)")]).udeb(),
        ];
        BinaryPackageTester::new(DefaultMtaDependencyNotListedFirst, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
