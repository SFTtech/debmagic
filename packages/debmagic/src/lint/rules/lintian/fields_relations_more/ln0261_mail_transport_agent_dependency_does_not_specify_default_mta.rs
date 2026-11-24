use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::{BINARY_DEP_FIELDS, SOURCE_RELATION_FIELDS};

pub struct MailTransportAgentDependencyDoesNotSpecifyDefaultMta;

declare_rule! {
    /// A relation names `mail-transport-agent` and not `default-mta`.
    ///
    /// Any alternative counts, including one after `|`, and an `:arch`
    /// qualifier is ignored. Binary fields are `Depends`, `Pre-Depends`,
    /// `Recommends`, and `Suggests`. Source fields are the build-dependency
    /// and build-conflict fields. Extra is the field and its unfolded value.
    /// Applies to debs, udebs, the Source tree, and the Source package.
    MailTransportAgentDependencyDoesNotSpecifyDefaultMta,
    code = "LN0261",
    tag = "mail-transport-agent-dependency-does-not-specify-default-mta",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MailTransportAgentDependencyDoesNotSpecifyDefaultMta {
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

impl BinaryPackageRule for MailTransportAgentDependencyDoesNotSpecifyDefaultMta {
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
            let has_agent = relation
                .alternatives()
                .any(|alternative| alternative.package() == "mail-transport-agent");
            let has_default = relation
                .alternatives()
                .any(|alternative| alternative.package() == "default-mta");
            if !has_agent || has_default {
                return None;
            }
            let value = fields.unfolded_value(field)?;
            Some(format!("{field}: {value}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &[(&str, &str)]) -> Vec<(&'static str, TestFile)> {
        let mut body = String::from("Source: example\n");
        for (name, value) in fields {
            body.push_str(name);
            body.push_str(": ");
            body.push_str(value);
            body.push('\n');
        }
        body.push_str("\nPackage: example\nArchitecture: all\nDescription: example\n extra\n");
        vec![("debian/control", TestFile::new(body))]
    }

    fn binary(items: &[(&str, &str)]) -> BinaryPackageCase {
        items.iter().fold(
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            |case, (name, value)| case.field(*name, *value),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(&[("Build-Depends", "debhelper")]),
            control(&[(
                "Build-Depends",
                "default-mta | mail-transport-agent, default-mta",
            )]),
            control(&[("Build-Depends", "default-mta")]),
            control(&[("Build-Conflicts", "default-mta | mail-transport-agent")]),
        ];
        let fail = vec![
            control(&[(
                "Build-Depends",
                "mail-transport-agent, libc6-dev, findutils",
            )]),
            control(&[
                ("Build-Depends-Indep", "exim4 | mail-transport-agent"),
                ("Build-Conflicts", "mail-transport-agent"),
            ]),
        ];
        SourceTreeTester::new(
            MailTransportAgentDependencyDoesNotSpecifyDefaultMta,
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
            binary(&[("Depends", "default-mta, exim4")]),
            binary(&[("Conflicts", "mail-transport-agent")]),
            binary(&[("Depends", "mail-transport-agent:any, default-mta")]),
        ];
        let fail = vec![
            binary(&[("Pre-Depends", "mail-transport-agent")]),
            binary(&[("Depends", "exim4 | postfix | mail-transport-agent")]),
            binary(&[
                ("Depends", "default-mta, mail-transport-agent"),
                ("Suggests", "exim4 | mail-transport-agent"),
            ]),
            binary(&[("Recommends", "mail-transport-agent")]).udeb(),
        ];
        BinaryPackageTester::new(
            MailTransportAgentDependencyDoesNotSpecifyDefaultMta,
            pass,
            fail,
        )
        .test_and_snapshot_with_suffix("binary");
    }
}
