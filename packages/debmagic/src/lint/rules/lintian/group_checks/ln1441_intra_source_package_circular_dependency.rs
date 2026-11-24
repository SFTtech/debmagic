use std::collections::{HashMap, HashSet};

use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::GroupContext;
use crate::lint::rule::GroupRule;

pub struct IntraSourcePackageCircularDependency;

declare_rule! {
    /// Reports installables built from the same source package that depend
    /// (or pre-depend) on each other in a cycle. The extra is the names of
    /// the packages of one cycle, sorted; a source package with two separate
    /// cycles gets one Diagnostic for each.
    ///
    /// A Group Rule: it needs the installables of the whole Group (the
    /// `.deb` files named in the `.changes`, or given beside it) and the
    /// Group's `.dsc`, so it does not run on a Group without a `.dsc`.
    /// Only strong dependencies (Pre-Depends and Depends) count; udebs are
    /// not looked at as the start of a cycle, but a dependency on one is an
    /// edge, as in lintian. The Diagnostic attaches to the Source package.
    IntraSourcePackageCircularDependency,
    code = "LN1441",
    tag = "intra-source-package-circular-dependency",
    default_selected = true,
    experimental = false,
}

impl GroupRule for IntraSourcePackageCircularDependency {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        // The packages that depend on another package of the Group, and what
        // they depend on.
        let mut nodes: Vec<String> = Vec::new();
        let mut edges: HashMap<String, Vec<String>> = HashMap::new();
        for installable in ctx.installables().iter().filter(|i| !i.is_udeb()) {
            let dependencies = ctx.direct_dependencies(installable);
            if dependencies.is_empty() {
                continue;
            }
            let name = installable.name().to_string();
            nodes.push(name.clone());
            edges.insert(
                name,
                dependencies
                    .iter()
                    .map(|dependency| dependency.name().to_string())
                    .collect(),
            );
        }
        // Two packages that depend on some package of the Group are the least
        // a cycle takes.
        if nodes.len() < 2 {
            return;
        }
        for mut component in strongly_connected_components(&nodes, &edges) {
            if component.len() < 2 {
                continue;
            }
            component.sort();
            ctx.diagnostic(Diagnostic::warn(component.join(" ")));
        }
    }
}

/// Tarjan's algorithm, as lintian's `Lintian::Check::GroupChecks::Graph`:
/// the components in the order they are completed. A name with no entry in
/// `edges` has no outgoing edge.
fn strongly_connected_components(
    nodes: &[String],
    edges: &HashMap<String, Vec<String>>,
) -> Vec<Vec<String>> {
    struct Search<'a> {
        edges: &'a HashMap<String, Vec<String>>,
        index: usize,
        stack: Vec<&'a str>,
        on_stack: HashSet<&'a str>,
        /// Per node: its index and the lowest index it reaches.
        info: HashMap<&'a str, (usize, usize)>,
        components: Vec<Vec<String>>,
    }

    impl<'a> Search<'a> {
        fn visit(&mut self, node: &'a str) {
            let index = self.index;
            self.index += 1;
            self.info.insert(node, (index, index));
            self.stack.push(node);
            self.on_stack.insert(node);

            let neighbours: &'a [String] = self.edges.get(node).map_or(&[], Vec::as_slice);
            let mut low = index;
            for neighbour in neighbours {
                let neighbour = neighbour.as_str();
                match self.info.get(neighbour).copied() {
                    None => {
                        self.visit(neighbour);
                        low = low.min(self.info[neighbour].1);
                    }
                    Some((neighbour_index, _)) if self.on_stack.contains(neighbour) => {
                        low = low.min(neighbour_index);
                    }
                    Some(_) => {}
                }
                self.info.insert(node, (index, low));
            }

            if index == low {
                let mut component = Vec::new();
                while let Some(member) = self.stack.pop() {
                    self.on_stack.remove(member);
                    component.push(member.to_string());
                    if member == node {
                        break;
                    }
                }
                self.components.push(component);
            }
        }
    }

    let mut search = Search {
        edges,
        index: 0,
        stack: Vec::new(),
        on_stack: HashSet::new(),
        info: HashMap::new(),
        components: Vec::new(),
    };
    for node in nodes {
        if !search.info.contains_key(node.as_str()) {
            search.visit(node);
        }
    }
    search.components
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";

    fn group(installables: &[(&str, &str)]) -> GroupCase {
        let mut case = GroupCase::new(DSC);
        for (name, fields) in installables {
            case = case.installable(*name, *fields);
        }
        case
    }

    #[test]
    fn group_rule() {
        let pass = vec![
            // Not a Group of installables at all.
            GroupCase::new(DSC),
            // A chain is not a cycle.
            group(&[("a", "Depends: b\n"), ("b", "Depends: c\n"), ("c", "")]),
            // A package depending on itself is not a cycle between packages.
            group(&[
                ("a", "Depends: a\n"),
                ("b", "Depends: a\n"),
                ("c", "Depends: a\n"),
            ]),
            // Weak relations do not count.
            group(&[("a", "Depends: b\n"), ("b", "Recommends: a\nSuggests: a\n")]),
            // A cycle through a package that is not in the Group.
            group(&[("a", "Depends: b, outside\n"), ("b", "Depends: outside\n")]),
            // The alternatives of a dependency are not each a dependency.
            group(&[
                ("a", "Depends: b | c\n"),
                ("b", "Depends: a | c\n"),
                ("c", ""),
            ])
            .installable("d", "Depends: a\n"),
            // A cycle that needs a udeb does not start from one.
            GroupCase::new(DSC)
                .installable("a", "Depends: b-udeb\n")
                .udeb("b-udeb", "Depends: a\n"),
        ];
        let fail = vec![
            group(&[("a", "Depends: b\n"), ("b", "Depends: a\n")]),
            group(&[
                ("a", "Pre-Depends: b\n"),
                ("b", "Depends: c (>= 1)\n"),
                ("c", "Depends: a\n"),
            ]),
            // Two separate cycles, and a package in neither.
            group(&[
                ("c1-a", "Depends: c1-b\n"),
                ("c1-b", "Depends: c1-c\n"),
                ("c1-c", "Depends: c1-a\n"),
                ("c2-a", "Depends: c2-b\n"),
                ("c2-b", "Depends: c2-c\n"),
                ("c2-c", "Depends: c2-a\n"),
                ("tail", "Depends: c1-a\n"),
            ]),
            // A strong dependency that is not the only one.
            group(&[
                ("a", "Depends: lib, b\n"),
                ("b", "Pre-Depends: a\n"),
                ("lib", ""),
            ]),
        ];
        GroupTester::new(IntraSourcePackageCircularDependency, pass, fail).test_and_snapshot();
    }

    #[test]
    fn components_are_found_in_completion_order() {
        let nodes: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        let edges = HashMap::from([
            ("a".to_string(), vec!["b".to_string()]),
            ("b".to_string(), vec!["c".to_string()]),
            ("c".to_string(), vec!["b".to_string()]),
        ]);
        assert_eq!(
            strongly_connected_components(&nodes, &edges),
            vec![
                vec!["c".to_string(), "b".to_string()],
                vec!["a".to_string()]
            ]
        );
    }
}
