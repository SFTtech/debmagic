use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::SOURCE_RELATION_FIELDS;
use super::architectures::invalid_architecture_string;

pub struct InvalidArchStringInSourceRelation;

declare_rule! {
    /// An architecture restriction in a source relation names an unknown architecture.
    ///
    /// `Build-Depends`, `Build-Depends-Indep`, `Build-Depends-Arch`, and the
    /// matching `Build-Conflicts` fields of the source paragraph. Each name in
    /// `[...]` (`!` already stripped) is this Tag when it is `all`, or neither
    /// a release architecture nor an architecture wildcard. A `:arch` qualifier
    /// is not a restriction. Extra is the architecture and `[field: alternative]`.
    InvalidArchStringInSourceRelation,
    code = "LN0200",
    tag = "invalid-arch-string-in-source-relation",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InvalidArchStringInSourceRelation {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx
            .debian_control()
            .map(|control| messages(control.source()))
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn messages(fields: &ControlParagraph) -> Vec<String> {
    SOURCE_RELATION_FIELDS
        .iter()
        .filter_map(|field| Some((*field, fields.relation(field)?)))
        .flat_map(|(field, relation)| {
            relation
                .alternatives()
                .flat_map(|alternative| {
                    alternative
                        .architectures()
                        .iter()
                        .filter(|architecture| invalid_architecture_string(architecture))
                        .map(|architecture| {
                            format!("{architecture} [{field}: {}]", alternative.text())
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

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

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(&[]),
            control(&[("Build-Depends", "debhelper-compat (= 14), libc6:any")]),
            control(&[(
                "Build-Depends",
                "foo [amd64], bar [linux-amd64], baz [any], other [linux-any], yet [any-amd64], pkg [any-powerpc], arch [!amd64], both [i386 any], alpha [alpha]",
            )]),
            control(&[("Build-Conflicts", "foo [amd64 i386], bar [!sparc]")]),
            // `all` as a package name is not an architecture restriction.
            control(&[("Build-Depends", "all [amd64]")]),
            // An installable Depends restriction is not a source relation.
            vec![(
                "debian/control",
                TestFile::new(
                    "Source: example\nBuild-Depends: debhelper\n\n\
                     Package: example\nArchitecture: all\nDepends: foo [all]\n\
                     Description: example\n extra\n",
                ),
            )],
        ];
        let fail = vec![
            control(&[
                (
                    "Build-Depends",
                    "foo [all], baz [source lintian-fake-arch], ok [amd64], other-pkg [kfreebsd-any]",
                ),
                ("Build-Depends-Indep", "bozzle [kfreebsd-i386]"),
                ("Build-Depends-Arch", "pkg [not-an-arch]"),
                ("Build-Conflicts", "bar [alpha test]"),
                ("Build-Conflicts-Arch", "neg [!not-a-real-arch]"),
            ]),
            control(&[("Build-Depends", "libc6:kfreebsd-i386 [kfreebsd-i386]")]),
        ];
        SourceTreeTester::new(InvalidArchStringInSourceRelation, pass, fail).test_and_snapshot();
    }
}
