use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::{Severity, SourceRule};

pub struct RedundantControlRelation;

declare_rule! {
    /// The named field lists multiple package relationships when one would be
    /// sufficient.
    ///
    /// Reads the source `Build-Depends`, `Build-Depends-Indep`,
    /// `Build-Conflicts`, and `Build-Conflicts-Indep` fields, and the installable
    /// `Pre-Depends`, `Depends`, `Recommends`, `Suggests`, `Breaks`,
    /// `Conflicts`, `Provides`, `Replaces`, and `Enhances` fields. Each field
    /// is loaded as its own relation, and every set of dependencies that imply
    /// one another is reported as lintian's `redundancies` does: the field
    /// name, then the set's members sorted and joined by `, `. Extra is
    /// `(in source paragraph)` or `(in section for pkg)`, then the field and
    /// the set. Pedantic, selected only when asked for.
    RedundantControlRelation,
    code = "LN1316",
    tag = "redundant-control-relation",
    default_selected = false,
    experimental = false,
}

const SOURCE_FIELDS: &[&str] = &[
    "Build-Depends",
    "Build-Depends-Indep",
    "Build-Conflicts",
    "Build-Conflicts-Indep",
];

const INSTALLABLE_FIELDS: &[&str] = &[
    "Pre-Depends",
    "Depends",
    "Recommends",
    "Suggests",
    "Breaks",
    "Conflicts",
    "Provides",
    "Replaces",
    "Enhances",
];

impl SourceRule for RedundantControlRelation {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let mut found =
            redundancy_messages(control.source(), SOURCE_FIELDS, "(in source paragraph)");
        for installable in control.installables() {
            found.extend(redundancy_messages(
                installable.fields(),
                INSTALLABLE_FIELDS,
                &format!("(in section for {})", installable.name()),
            ));
        }
        for message in found {
            ctx.diagnostic(
                Diagnostic::warn(message)
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path: std::path::PathBuf::from("debian/control"),
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

fn redundancy_messages(fields: &ControlParagraph, names: &[&str], explainer: &str) -> Vec<String> {
    names
        .iter()
        .filter_map(|name| fields.relation(name).map(|relation| (name, relation)))
        .flat_map(|(name, relation)| {
            redundant_sets(&relation)
                .into_iter()
                .map(move |set| format!("{explainer} {name} {set}"))
        })
        .collect()
}

/// lintian's `Lintian::Relation::redundancies` for an AND relation: each
/// dependency is compared with every later one, and a pair that implies in
/// either direction joins a set. The earliest member keys the set. Returns
/// each set as its members sorted and joined by `, `.
fn redundant_sets(relation: &PackageRelation) -> Vec<String> {
    // (as lintian prints the branch, text the implication check reads)
    let branches: Vec<(String, String)> = relation
        .dependencies()
        .iter()
        .filter(|dependency| !dependency.is_empty())
        .map(|dependency| {
            let text = dependency
                .iter()
                .map(|alternative| alternative.text())
                .collect::<Vec<_>>()
                .join(" | ");
            (PackageRelation::parse(&text).to_lintian_string(), text)
        })
        .collect();

    let mut sets: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut seen: HashMap<String, String> = HashMap::new();
    for (i, (one, one_text)) in branches.iter().enumerate() {
        for (two, two_text) in &branches[i + 1..] {
            let implies = PackageRelation::parse(one_text).satisfies(two_text)
                || PackageRelation::parse(two_text).satisfies(one_text);
            if !implies {
                continue;
            }
            if let Some(key) = seen.get(one).cloned() {
                sets.entry(key.clone()).or_default().insert(two.clone());
                seen.insert(two.clone(), key);
            } else if let Some(key) = seen.get(two).cloned() {
                sets.entry(key.clone()).or_default().insert(one.clone());
                seen.insert(one.clone(), key);
            } else {
                sets.entry(one.clone()).or_default().insert(two.clone());
                seen.insert(two.clone(), one.clone());
            }
        }
    }

    sets.into_iter()
        .map(|(key, others)| {
            let mut members: Vec<String> = others.into_iter().collect();
            members.push(key);
            members.sort();
            members.join(", ")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn packaged(source_relations: &str, binary_relations: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n{source_relations}\
                 \n\
                 Package: example\nArchitecture: all\n{binary_relations}\
                 Description: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn source() {
        let pass = vec![
            packaged("", ""),
            packaged("Build-Depends: foo, bar\n", ""),
            packaged("Build-Depends: foo | bar\n", ""),
            packaged("Build-Depends: foo (>= 1), foo (<< 2)\n", ""),
            packaged("", "Depends: a, b\n"),
        ];
        let fail = vec![
            packaged("Build-Depends: foobar, foobar\n", ""),
            packaged("Build-Depends-Indep: foobar, foobar\n", ""),
            packaged("Build-Depends: debhelper, debhelper (>= 14~)\n", ""),
            packaged("Build-Conflicts: bar, bar [i386]\n", ""),
            packaged(
                "",
                "Suggests: baz, baz (= 1.0), baz (>= 2.0)\nDepends: ${misc:Depends}, ${misc:Depends}\n",
            ),
        ];
        SourceTreeTester::new(RedundantControlRelation, pass, fail).test_and_snapshot();
    }
}
