use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::SourceRule;

use super::sections::{control_location, tests_control_sections};

pub struct TestsuiteDependencyHasUnparsableElements;

declare_rule! {
    /// A paragraph of `debian/tests/control` has a `Depends` element that the
    /// relation grammar cannot parse.
    ///
    /// The unfolded `Depends` is parsed as a package relation. Each alternative
    /// the predicate grammar rejects is one finding, in sorted order. The autopkgtest
    /// specials `@`, `@builddeps@` and `@recommends@` are accepted and not
    /// reported. Extra is the literal text in double quotes. The Location is the
    /// `Depends` line. Applies to the Source tree and Source package.
    TestsuiteDependencyHasUnparsableElements,
    code = "LN1139",
    tag = "testsuite-dependency-has-unparsable-elements",
    default_selected = true,
    experimental = false,
}

/// Predicates autopkgtest allows although they are not package names.
const SPECIAL_DEPENDS: [&str; 3] = ["@", "@builddeps@", "@recommends@"];

impl SourceRule for TestsuiteDependencyHasUnparsableElements {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        let mut hits = Vec::new();
        for section in &sections {
            if !section.declares("Depends") {
                continue;
            }
            let line = section.line("Depends");
            let depends = section.unfolded_value("Depends");
            for literal in PackageRelation::parse(depends.trim()).unparsable_predicates() {
                if !SPECIAL_DEPENDS.contains(&literal.as_str()) {
                    hits.push((format!("\"{literal}\""), line));
                }
            }
        }
        for (extra, line) in hits {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(control_location(line)));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/tests/control", TestFile::new(body))]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("Tests: smoke\n"),
            tree("Tests: smoke\nDepends: @\n"),
            tree("Tests: smoke\nDepends: @builddeps@, @recommends@\n"),
            tree("Tests: smoke\nDepends: python3 (>= 3.9), libfoo-dev | libbar-dev [amd64]\n"),
            tree("Tests: smoke\nDepends: ${misc:Depends}, ${python3:Depends}\n"),
        ];
        let fail = vec![
            tree("Tests: smoke\nDepends: missing a comma\n"),
            tree("Tests: smoke\nDepends: foo, bar baz, @builddeps@\n"),
            tree("Tests: a\nDepends: @\n\nTests: b\nDepends: zed qux, alpha beta\n"),
        ];
        SourceTreeTester::new(TestsuiteDependencyHasUnparsableElements, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("Tests: smoke\nDepends: @\n"))];
        let fail = vec![SourcePackageCase::from(tree(
            "Tests: smoke\nDepends: missing a comma\n",
        ))];
        SourcePackageTester::new(TestsuiteDependencyHasUnparsableElements, pass, fail)
            .test_and_snapshot();
    }
}
