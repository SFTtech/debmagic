use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::SourceRule;

pub struct RedundantBuildPrerequisites;

declare_rule! {
    /// The build prerequisites repeat or imply one another.
    ///
    /// The fields are combined as lintian does for `Build-Depends` alone, with
    /// `Build-Depends-Indep`, and with `Build-Depends-Arch`. Within each
    /// combination, dependencies that imply one another (each satisfies the
    /// other, as `satisfies` decides) form one set. Extra is the members of a
    /// set, sorted and joined with `, `. The fields are the `.dsc` paragraph of
    /// a source package (lintian reads the relations there) and the source
    /// paragraph of `debian/control` on a source tree. Applies to source trees
    /// and source packages.
    RedundantBuildPrerequisites,
    code = "LN0932",
    tag = "redundant-build-prerequisites",
    default_selected = true,
    experimental = false,
}

impl SourceRule for RedundantBuildPrerequisites {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx.relation_fields().map(messages).unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

/// The field combinations lintian checks, in its order.
const COMBINATIONS: &[&[&str]] = &[
    &["Build-Depends"],
    &["Build-Depends", "Build-Depends-Indep"],
    &["Build-Depends", "Build-Depends-Arch"],
];

fn messages(fields: &ControlParagraph) -> Vec<String> {
    COMBINATIONS
        .iter()
        .flat_map(|names| {
            let combined = names
                .iter()
                .fold(PackageRelation::default(), |combined, name| {
                    combined.and(&fields.relation(name).unwrap_or_default())
                });
            redundant_sets(&combined)
        })
        .collect()
}

/// lintian's `Lintian::Relation::redundancies` for an AND relation: each
/// dependency is compared with every later one, and a pair that implies in
/// either direction joins a set. The earliest member keys the set.
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
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

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

    /// lintian's `Check/Fields/PackageRelations.pm` `source` sub runs
    /// `redundancies` on the AND of Build-Depends alone, with Build-Depends-Indep,
    /// and with Build-Depends-Arch, and each returned set becomes one hint.
    #[test]
    fn redundant_sets_per_field_combination() {
        let pass = vec![
            control(&[]),
            control(&[("Build-Depends", "debhelper-compat (= 14), foo")]),
            control(&[
                ("Build-Depends", "foo (>= 1)"),
                ("Build-Depends-Indep", "bar"),
            ]),
            control(&[
                ("Build-Depends", "foo (>= 1)"),
                ("Build-Depends-Arch", "foo (<< 2)"),
            ]),
            control(&[
                ("Build-Depends-Indep", "foo"),
                ("Build-Depends-Arch", "foo"),
            ]),
        ];
        let fail = vec![
            control(&[(
                "Build-Depends",
                "perl (>= 5.10), perl (>= 5.10) | libmodule-build-perl",
            )]),
            control(&[
                ("Build-Depends", "perl"),
                ("Build-Depends-Indep", "perl (>= 5.0)"),
            ]),
            control(&[
                ("Build-Depends", "foo, foo (>= 1)"),
                ("Build-Depends-Arch", "foo (<< 4)"),
            ]),
            control(&[
                ("Build-Depends", "foo (<< 4) [!amd64 !i386]"),
                ("Build-Depends-Arch", "foo (= 3)"),
            ]),
        ];
        SourceTreeTester::new(RedundantBuildPrerequisites, pass, fail).test_and_snapshot();
    }

    fn dsc(fields: &str) -> Dsc {
        Dsc::parse(&format!("Source: example\nVersion: 1.0\n{fields}"))
            .expect("generated .dsc paragraph")
    }

    /// lintian's `Relation::redundancies` runs on the `.dsc` relations of a
    /// source package (`Processable::Source`), not on `debian/control`.
    #[test]
    fn source_package_relations_come_from_the_dsc() {
        let pass = vec![
            SourcePackageCase::from(control(&[("Build-Depends", "foo, foo (>= 1)")]))
                .dsc(dsc("Build-Depends: foo\n")),
        ];
        let fail = vec![
            SourcePackageCase::from(control(&[])).dsc(dsc("Build-Depends: foo, foo (>= 1)\n")),
        ];
        SourcePackageTester::new(RedundantBuildPrerequisites, pass, fail).test_and_snapshot();
    }
}
