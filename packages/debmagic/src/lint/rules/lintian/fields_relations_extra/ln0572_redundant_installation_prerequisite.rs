use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::DebianControl;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::comma_list;

pub struct RedundantInstallationPrerequisite;

declare_rule! {
    /// A weaker dependency field repeats what a stronger field of the same installable requires.
    ///
    /// Fields from strongest to weakest are `Pre-Depends`, `Depends`,
    /// `Recommends`, `Suggests`. For each installable, each field it declares
    /// is taken as the stronger one in turn, and every comma-separated entry
    /// of each weaker field is tested with relation satisfaction (the same
    /// test as `conflicts-with-dependency`). Extra is `(in section for
    /// <installable>)`, the weaker field, `->`, the stronger field, and the
    /// entry as written.
    RedundantInstallationPrerequisite,
    code = "LN0572",
    tag = "redundant-installation-prerequisite",
    default_selected = true,
    experimental = false,
}

const ORDERED_FIELDS: &[&str] = &["Pre-Depends", "Depends", "Recommends", "Suggests"];

impl SourceRule for RedundantInstallationPrerequisite {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let hits = ctx.debian_control().map(messages).unwrap_or_default();
        for (message, line) in hits {
            ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                path: PathBuf::from("debian/control"),
                line,
                column: None,
            }));
        }
    }
}

/// Each hit with the line of the stronger field, which lintian points at.
fn messages(control: &DebianControl) -> Vec<(String, Option<u32>)> {
    let mut hits = Vec::new();
    for installable in control.installables() {
        let fields = installable.fields();
        for (index, stronger) in ORDERED_FIELDS.iter().enumerate() {
            let Some(relation) = fields.relation(stronger) else {
                continue;
            };
            let line = fields.position(stronger);
            for weaker in &ORDERED_FIELDS[index + 1..] {
                for prerequisite in comma_list(fields, weaker) {
                    if relation.satisfies(&prerequisite) {
                        let message = format!(
                            "(in section for {}) {weaker} -> {stronger} {prerequisite}",
                            installable.name()
                        );
                        hits.push((message, line));
                    }
                }
            }
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::super::testing::raw;
    use super::*;

    fn installable(fields: &str) -> Vec<(&'static str, TestFile)> {
        raw(&format!("Source: example\n\nPackage: example\n{fields}"))
    }

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            installable("Depends: foo\nRecommends: bar\nSuggests: baz\n"),
            installable("Depends: foo (>= 2)\nRecommends: foo (>= 3)\n"),
            // A stronger field that is only a weaker one does not matter.
            installable("Recommends: foo\nDepends: bar\n"),
            installable("Depends: foo | bar\nSuggests: foo\n"),
            installable("Depends: foo [amd64]\nRecommends: foo\n"),
        ];
        let fail = vec![
            installable(
                "Pre-Depends: gawk | awk\nDepends: gawk | awk, foo\nRecommends: foo, dpkg, null\n\
                 Suggests: foo, bar | baz\n",
            ),
            installable("Depends: foo (>= 2)\nSuggests: foo (>= 1), foo\n"),
            installable(
                "Depends: ${shlibs:Depends}, bar | baz\nRecommends: ${shlibs:Depends}, bar | baz\n",
            ),
            raw(
                "Source: example\n\nPackage: one\nDepends: x\nSuggests: x\n\nPackage: two\nRecommends: y\nSuggests: y\n",
            ),
        ];
        SourceTreeTester::new(RedundantInstallationPrerequisite, pass, fail).test_and_snapshot();
    }
}
