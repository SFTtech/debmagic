use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

pub struct BogusPythonPrerequisite;

declare_rule! {
    /// A relation satisfies `python-is-python3:any` or `python-dev-is-python3:any`.
    ///
    /// Source fields are `Build-Depends`, `Build-Depends-Indep`, and
    /// `Build-Depends-Arch`. Binary fields are `Depends`, `Pre-Depends`, and
    /// `Recommends`. The source package `what-is-python` is exempt. Extra is
    /// the field and `(satisfies <prerequisite>)`, with prerequisites in that
    /// order and fields in the order above. Applies to the Source tree, the
    /// Source package, debs, and udebs.
    BogusPythonPrerequisite,
    code = "LN0476",
    tag = "bogus-python-prerequisite",
    default_selected = true,
    experimental = false,
}

const UNWANTED: &[&str] = &["python-is-python3:any", "python-dev-is-python3:any"];
const SOURCE_FIELDS: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];
const BINARY_FIELDS: &[&str] = &["Depends", "Pre-Depends", "Recommends"];

impl SourceRule for BogusPythonPrerequisite {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = {
            let Some(control) = ctx.debian_control() else {
                return;
            };
            let fields = control.source();
            if source_name(fields) == Some("what-is-python") {
                return;
            }
            prerequisite_messages(fields, SOURCE_FIELDS)
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

impl BinaryPackageRule for BogusPythonPrerequisite {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.source_name() == "what-is-python" {
            return;
        }
        let messages = prerequisite_messages(ctx.installation_control().fields(), BINARY_FIELDS);
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn source_name(fields: &ControlParagraph) -> Option<&str> {
    fields
        .get("Source")
        .and_then(|value| value.split_whitespace().next())
        .filter(|name| !name.is_empty())
}

fn prerequisite_messages(fields: &ControlParagraph, names: &[&str]) -> Vec<String> {
    let mut messages = Vec::new();
    for unwanted in UNWANTED {
        for name in names {
            let Some(relation) = fields.relation(name) else {
                continue;
            };
            if relation.satisfies(unwanted) {
                messages.push(format!("{name} (satisfies {unwanted})"));
            }
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(source: &str, fields: &[(&str, &str)]) -> Vec<(&'static str, TestFile)> {
        let mut body = format!("Source: {source}\n");
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
            control("example", &[("Build-Depends", "debhelper")]),
            control("example", &[("Build-Depends", "foo | python-is-python3")]),
            control("example", &[("Build-Depends", "python-is-python3 [amd64]")]),
            control(
                "example",
                &[("Build-Depends", "python-is-python3 <!nocheck>")],
            ),
            control(
                "example",
                &[(
                    "Build-Depends",
                    "python-is-python3:any | python-dev-is-python3:any",
                )],
            ),
            control(
                "example",
                &[(
                    "Build-Conflicts",
                    "python-is-python3, python-dev-is-python3",
                )],
            ),
            control(
                "what-is-python",
                &[("Build-Depends", "python-is-python3, python-dev-is-python3")],
            ),
            control(
                "what-is-python (1.0)",
                &[("Build-Depends", "python-is-python3")],
            ),
        ];
        let fail = vec![
            control("example", &[("Build-Depends", "python-dev-is-python3")]),
            control("example", &[("Build-Depends", "python-is-python3:any")]),
            control("example", &[("Build-Depends-Indep", "python-is-python3")]),
            control(
                "example",
                &[("Build-Depends-Arch", "python-dev-is-python3 (>= 1)")],
            ),
            control(
                "what-is-python-depends",
                &[(
                    "Build-Depends",
                    "debhelper-compat (= 14), python-is-python3",
                )],
            ),
            control(
                "example",
                &[
                    ("Build-Depends", "python-is-python3, python-dev-is-python3"),
                    ("Build-Depends-Arch", "python-dev-is-python3:any"),
                ],
            ),
        ];
        SourceTreeTester::new(BogusPythonPrerequisite, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_binary() {
        let pass = vec![
            binary(&[]),
            binary(&[("Depends", "python3")]),
            binary(&[("Depends", "foo | python-is-python3")]),
            binary(&[("Depends", "python-is-python3 [amd64]")]),
            binary(&[("Conflicts", "python-is-python3")]),
            binary(&[("Depends", "python-is-python3")]).field("Source", "what-is-python"),
            binary(&[("Depends", "python-is-python3")]).field("Source", "what-is-python (1.0)"),
            binary(&[("Depends", "python-is-python3")]).package("what-is-python"),
        ];
        let fail = vec![
            binary(&[("Depends", "python-is-python3")]),
            binary(&[("Pre-Depends", "python-dev-is-python3:any")]),
            binary(&[("Recommends", "python-is-python3, python-dev-is-python3")]),
            binary(&[
                ("Depends", "python-is-python3"),
                ("Pre-Depends", "python-dev-is-python3"),
            ]),
            binary(&[("Depends", "python-is-python3 (>= 1)")]).udeb(),
            binary(&[("Depends", "python-is-python3")])
                .package("what-is-python")
                .field("Source", "other"),
        ];
        BinaryPackageTester::new(BogusPythonPrerequisite, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
