use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::SOURCE_RELATION_FIELDS;
use super::essential::is_essential;

pub struct BuildDependsOnEssentialPackageWithoutUsingVersion;

declare_rule! {
    /// A source relation names an essential package and gives no version constraint.
    ///
    /// Build-dependency and build-conflict fields of the source paragraph.
    /// Essential packages are lintian's `fields/essential` list. `dash` is
    /// excluded. Presence of a `(op version)` constraint is enough; versions
    /// are not compared. Extra is the field and the alternative as written.
    BuildDependsOnEssentialPackageWithoutUsingVersion,
    code = "LN0207",
    tag = "build-depends-on-essential-package-without-using-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for BuildDependsOnEssentialPackageWithoutUsingVersion {
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
                .filter(|alternative| {
                    alternative.package() != "dash"
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
        let installable_only = vec![(
            "debian/control",
            TestFile::new(
                "Source: example\n\n\
                 Package: example\nArchitecture: all\nDepends: bash\n\
                 Description: example\n extra\n",
            ),
        )];
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(&[]),
            control(&[
                (
                    "Build-Depends",
                    "make, bash (>= 5), dash, libc6, foo | dash",
                ),
                ("Build-Conflicts", "dash"),
            ]),
            installable_only,
        ];
        let fail = vec![
            control(&[
                (
                    "Build-Depends",
                    "bash, findutils (>= 4), dash, foo | tar, diffutils, bash [amd64], findutils",
                ),
                ("Build-Depends-Indep", "grep"),
                ("Build-Depends-Arch", "gzip"),
                ("Build-Conflicts", "dpkg"),
            ]),
            control(&[("Build-Depends", "bash (1.0)")]),
        ];
        SourceTreeTester::new(
            BuildDependsOnEssentialPackageWithoutUsingVersion,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
