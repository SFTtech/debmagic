use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::ln0688_testsuite_depends_on_obsolete_package::obsolete_messages;
use super::sections::tests_control_sections;

pub struct TestsuiteOredDependsOnObsoletePackage;

declare_rule! {
    /// A test `Depends` in `debian/tests/control` has an obsolete package
    /// among ORed alternatives that include a current package, and that
    /// obsolete alternative is not the first one. Reported at info severity
    /// with extra `Depends: <alternative>` and ` => <replacement>` when the
    /// data file names one. There is no Location: lintian's hint has no
    /// pointer, so the hint carries no `[path:line]`.
    ///
    /// The split and the rule are those of LN0688, which reports the other
    /// case (an obsolete first alternative, or one where every alternative is
    /// obsolete) as `testsuite-depends-on-obsolete-package`.
    TestsuiteOredDependsOnObsoletePackage,
    code = "LN1692",
    tag = "testsuite-ored-depends-on-obsolete-package",
    default_selected = false,
    experimental = false,
}

impl SourceRule for TestsuiteOredDependsOnObsoletePackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        for message in obsolete_messages(&sections, true) {
            ctx.diagnostic(Diagnostic::warn(message).with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    fn tree(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/tests/control", TestFile::new(body))]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no tests"))],
            // The obsolete alternative is first, or every alternative is obsolete.
            tree("Test-Command: true\nDepends: gaim | xbase-clients\n"),
            tree("Test-Command: true\nDepends: gaim\n"),
            tree("Test-Command: true\nDepends: new-package | libc6\n"),
        ];
        let fail = vec![
            tree("Test-Command: true\nDepends: current | gaim\n"),
            tree("Test-Command: true\nDepends: new-package | xbase-clients\n"),
            tree("Test-Command: true\nDepends: @, libc6, current | xbase-clients (>= 0.1)\n"),
        ];
        SourceTreeTester::new(TestsuiteOredDependsOnObsoletePackage, pass, fail)
            .test_and_snapshot();
    }

    /// Lintian's hint for an obsolete alternative behind a current one names no
    /// pointer, so the Diagnostic has no Location and its hint carries no `[path:line]`.
    #[test]
    fn hint_has_no_location() {
        let tester = SourceTreeTester::new(
            TestsuiteOredDependsOnObsoletePackage,
            Vec::<TestCase>::new(),
            Vec::<TestCase>::new(),
        );
        for files in [
            tree("Test-Command: true\nDepends: current | gaim\n"),
            tree("Test-Command: true\nDepends: @, libc6, current | xbase-clients (>= 0.1)\n"),
        ] {
            let diagnostics = tester.diagnostics(files);
            assert!(!diagnostics.is_empty());
            for diagnostic in diagnostics {
                assert!(diagnostic.location.path.as_os_str().is_empty());
            }
        }
    }
}
