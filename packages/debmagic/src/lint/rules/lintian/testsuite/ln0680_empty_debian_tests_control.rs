use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::sections::{control_location, tests_control_sections};

pub struct EmptyDebianTestsControl;

declare_rule! {
    /// `debian/tests/control` has no paragraph.
    ///
    /// The file must be valid UTF-8 and parse as Deb822 with comments at end of
    /// line; an empty or fully commented file has no paragraph. Extra is empty.
    /// The Location is `debian/tests/control`. Applies to the Source tree and
    /// Source package.
    EmptyDebianTestsControl,
    code = "LN0680",
    tag = "empty-debian-tests-control",
    default_selected = true,
    experimental = false,
}

impl SourceRule for EmptyDebianTestsControl {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        if sections.is_empty() {
            ctx.diagnostic(Diagnostic::error("").with_location(control_location(None)));
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
            vec![("README", TestFile::new("no tests"))],
            tree("Tests: smoke\n"),
            tree("# comment\nTests: smoke\n"),
        ];
        let fail = vec![
            tree(""),
            tree("\n\n"),
            tree("#Test-Command: everything is commented out\n#Depends: @\n"),
        ];
        SourceTreeTester::new(EmptyDebianTestsControl, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("Tests: smoke\n"))];
        let fail = vec![SourcePackageCase::from(tree(""))];
        SourcePackageTester::new(EmptyDebianTestsControl, pass, fail).test_and_snapshot();
    }
}
