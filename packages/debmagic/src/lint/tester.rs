use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use super::context::LintContext;
use super::diagnostic::Diagnostic;
use super::rule::Rule;
use super::run::format_diagnostic;
use super::subject::Subject;

/// One Subject described as a map of relative paths to file contents.
#[derive(Clone, Debug, Default)]
pub struct TestCase {
    files: BTreeMap<String, String>,
}

impl TestCase {
    pub fn source_tree<P, C>(files: impl IntoIterator<Item = (P, C)>) -> Self
    where
        P: Into<String>,
        C: Into<String>,
    {
        Self {
            files: files
                .into_iter()
                .map(|(path, content)| (path.into(), content.into()))
                .collect(),
        }
    }

    fn describe(&self) -> String {
        if self.files.is_empty() {
            return "<empty tree>".to_string();
        }
        self.files.keys().cloned().collect::<Vec<_>>().join(", ")
    }
}

impl<P, C> From<Vec<(P, C)>> for TestCase
where
    P: Into<String>,
    C: Into<String>,
{
    fn from(files: Vec<(P, C)>) -> Self {
        Self::source_tree(files)
    }
}

impl<const N: usize, P, C> From<[(P, C); N]> for TestCase
where
    P: Into<String>,
    C: Into<String>,
{
    fn from(files: [(P, C); N]) -> Self {
        Self::source_tree(files)
    }
}

enum TestResult {
    Passed,
    Failed,
}

/// Rule-level test harness inspired by oxlint's `Tester`.
///
/// Pass cases must emit no Diagnostics; fail cases must emit at least one.
/// `test_and_snapshot` records fail-case Diagnostics with insta.
pub struct Tester {
    rule: Box<dyn Rule>,
    expect_pass: Vec<TestCase>,
    expect_fail: Vec<TestCase>,
    snapshot: String,
}

impl Tester {
    pub fn new<R, P, F, C>(rule: R, expect_pass: P, expect_fail: F) -> Self
    where
        R: Rule + 'static,
        P: IntoIterator<Item = C>,
        F: IntoIterator<Item = C>,
        C: Into<TestCase>,
    {
        Self {
            rule: Box::new(rule),
            expect_pass: expect_pass.into_iter().map(Into::into).collect(),
            expect_fail: expect_fail.into_iter().map(Into::into).collect(),
            snapshot: String::new(),
        }
    }

    pub fn test(&mut self) {
        let unexpected_fails = self.test_pass();
        let unexpected_passes = self.test_fail();

        if !unexpected_fails.is_empty() {
            println!(
                "{}",
                format_test_failures("expected to pass, but failed", &unexpected_fails)
            );
        }
        if !unexpected_passes.is_empty() {
            println!(
                "{}",
                format_test_failures("expected to fail, but passed", &unexpected_passes)
            );
        }

        assert!(
            unexpected_fails.is_empty() && unexpected_passes.is_empty(),
            "Some tests failed for Rule {} [{}] (see output above)",
            self.rule.code(),
            self.rule.tag(),
        );
    }

    pub fn test_and_snapshot(&mut self) {
        self.test();
        self.snapshot();
    }

    fn snapshot(&self) {
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        settings.set_omit_expression(true);
        let name = format!(
            "{}_{}",
            self.rule.code(),
            self.rule.tag().as_str().replace('-', "_")
        );
        settings.bind(|| {
            insta::assert_snapshot!(name, self.snapshot);
        });
    }

    fn test_pass(&mut self) -> Vec<TestFailure> {
        let mut unexpected = Vec::new();
        let mut output_index = 0;
        for case in self.expect_pass.clone() {
            let result = self.run(&case);
            if !matches!(result, TestResult::Passed) {
                unexpected.push(TestFailure::ExpectedToPass {
                    case: case.describe(),
                    diagnostic: self.snapshot[output_index..].to_string(),
                });
            }
            output_index = self.snapshot.len();
        }
        unexpected
    }

    fn test_fail(&mut self) -> Vec<TestFailure> {
        let mut unexpected = Vec::new();
        for case in self.expect_fail.clone() {
            let result = self.run(&case);
            if !matches!(result, TestResult::Failed) {
                unexpected.push(TestFailure::ExpectedToFail {
                    case: case.describe(),
                });
            }
        }
        unexpected
    }

    fn run(&mut self, case: &TestCase) -> TestResult {
        let root = materialize(case);
        let subject = Subject::SourceTree(root.path().to_path_buf());
        let mut ctx = LintContext::new(&subject, self.rule.as_ref());
        self.rule.run(&mut ctx);
        let mut diagnostics = ctx.into_diagnostics();
        for diagnostic in &mut diagnostics {
            relativize_location(diagnostic, root.path());
        }

        if diagnostics.is_empty() {
            return TestResult::Passed;
        }

        for diagnostic in diagnostics {
            writeln!(self.snapshot, "{}", format_diagnostic(&diagnostic))
                .expect("writing to string never fails");
        }
        TestResult::Failed
    }
}

enum TestFailure {
    ExpectedToPass { case: String, diagnostic: String },
    ExpectedToFail { case: String },
}

fn materialize(case: &TestCase) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("creating temp Subject tree");
    for (relative, content) in &case.files {
        let path = dir.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|error| panic!("creating {}: {error}", parent.display()));
        }
        fs::write(&path, content)
            .unwrap_or_else(|error| panic!("writing {}: {error}", path.display()));
    }
    dir
}

fn relativize_location(diagnostic: &mut Diagnostic, root: &Path) {
    if let Ok(relative) = diagnostic.location.path.strip_prefix(root) {
        diagnostic.location.path = PathBuf::from(relative);
    }
}

fn format_test_failures(reason: &str, failures: &[TestFailure]) -> String {
    let count = failures.len();
    let mut output = String::new();
    writeln!(
        output,
        "\n{count} test case{} {reason}:\n",
        if count == 1 { "" } else { "s" }
    )
    .expect("writing to string never fails");

    for (index, failure) in failures.iter().enumerate() {
        match failure {
            TestFailure::ExpectedToPass { case, diagnostic } => {
                writeln!(output, " {:>2}. {case}", index + 1)
                    .expect("writing to string never fails");
                writeln!(output, "     Diagnostic:\n{diagnostic}")
                    .expect("writing to string never fails");
            }
            TestFailure::ExpectedToFail { case } => {
                writeln!(output, " {:>2}. {case}", index + 1)
                    .expect("writing to string never fails");
            }
        }
    }
    output
}
