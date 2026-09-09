use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use super::context::{
    BinaryPackageContext, BinaryPackageData, SourceContext, SourcePackageData, SourceTreeData,
};
use super::diagnostic::Diagnostic;
use super::dsc::Dsc;
use super::file_tree::{Entry, EntryKind, FileTree};
use super::file_type::FileType;
use super::rule::{BinaryPackageRule, SourceRule};
use super::run::format_diagnostic;

/// One Source tree described as a map of relative paths to file contents.
#[derive(Clone, Debug, Default)]
pub struct TestCase {
    files: BTreeMap<String, TestFile>,
}

/// One fixture Entry: bytes plus optional File type and Entry kind.
#[derive(Clone, Debug)]
pub struct TestFile {
    content: String,
    file_type: Option<FileType>,
    kind: Option<EntryKind>,
}

impl TestFile {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            file_type: None,
            kind: None,
        }
    }

    pub fn file_type(mut self, file_type: FileType) -> Self {
        self.file_type = Some(file_type);
        self
    }

    pub fn kind(mut self, kind: EntryKind) -> Self {
        self.kind = Some(kind);
        self
    }
}

impl From<&str> for TestFile {
    fn from(content: &str) -> Self {
        Self::new(content)
    }
}

impl From<String> for TestFile {
    fn from(content: String) -> Self {
        Self::new(content)
    }
}

impl TestCase {
    pub fn source_tree<P, C>(files: impl IntoIterator<Item = (P, C)>) -> Self
    where
        P: Into<String>,
        C: Into<TestFile>,
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

    fn file_tree(&self) -> FileTree {
        let entries = self
            .files
            .iter()
            .map(|(path, file)| {
                Entry::fixture(
                    path,
                    file.kind.unwrap_or(EntryKind::RegularFile),
                    file.file_type,
                    Some(file.content.as_bytes().to_vec()),
                )
            })
            .collect();
        FileTree::from_entries(None, entries)
    }
}

impl<P, C> From<Vec<(P, C)>> for TestCase
where
    P: Into<String>,
    C: Into<TestFile>,
{
    fn from(files: Vec<(P, C)>) -> Self {
        Self::source_tree(files)
    }
}

impl<const N: usize, P, C> From<[(P, C); N]> for TestCase
where
    P: Into<String>,
    C: Into<TestFile>,
{
    fn from(files: [(P, C); N]) -> Self {
        Self::source_tree(files)
    }
}

enum TestResult {
    Passed,
    Failed,
}

/// Source-tree Rule-level test harness inspired by oxlint's `Tester`.
///
/// Pass cases must emit no Diagnostics; fail cases must emit at least one.
/// `test_and_snapshot` records fail-case Diagnostics with insta.
pub struct SourceTreeTester {
    rule: Box<dyn SourceRule>,
    expect_pass: Vec<TestCase>,
    expect_fail: Vec<TestCase>,
    snapshot: String,
}

impl SourceTreeTester {
    pub fn new<R, P, F, C>(rule: R, expect_pass: P, expect_fail: F) -> Self
    where
        R: SourceRule + 'static,
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
        let data = SourceTreeData::with_file_tree(root.path().to_path_buf(), case.file_tree());
        let mut ctx = SourceContext::bind_tree(&data, self.rule.as_ref());
        self.rule.run(&mut ctx);
        let mut diagnostics = ctx
            .finish()
            .expect("SourceTreeTester File tree does not call the File type producer");
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

/// One Binary package described as Data files and a package name.
#[derive(Clone, Debug)]
pub struct BinaryPackageCase {
    files: TestCase,
    package: String,
}

impl BinaryPackageCase {
    pub fn package(mut self, name: impl Into<String>) -> Self {
        self.package = name.into();
        self
    }

    fn describe(&self) -> String {
        let files = self.files.describe();
        if self.package == "example" {
            files
        } else {
            format!("package {}: {files}", self.package)
        }
    }
}

impl From<TestCase> for BinaryPackageCase {
    fn from(files: TestCase) -> Self {
        Self {
            files,
            package: "example".to_string(),
        }
    }
}

impl<P, C> From<Vec<(P, C)>> for BinaryPackageCase
where
    P: Into<String>,
    C: Into<TestFile>,
{
    fn from(files: Vec<(P, C)>) -> Self {
        BinaryPackageCase::from(TestCase::from(files))
    }
}

/// Binary-package Rule-level test harness: Data files fixtures, not a `.deb`.
pub struct BinaryPackageTester {
    rule: Box<dyn BinaryPackageRule>,
    expect_pass: Vec<BinaryPackageCase>,
    expect_fail: Vec<BinaryPackageCase>,
    snapshot: String,
}

impl BinaryPackageTester {
    pub fn new<R, P, F, C>(rule: R, expect_pass: P, expect_fail: F) -> Self
    where
        R: BinaryPackageRule + 'static,
        P: IntoIterator<Item = C>,
        F: IntoIterator<Item = C>,
        C: Into<BinaryPackageCase>,
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

    fn run(&mut self, case: &BinaryPackageCase) -> TestResult {
        let data = BinaryPackageData::with_data_files(
            PathBuf::from("example.deb"),
            case.package.clone(),
            case.files.file_tree(),
        );
        let mut ctx = BinaryPackageContext::bind(&data, self.rule.as_ref());
        self.rule.run(&mut ctx);
        let diagnostics = ctx
            .finish()
            .expect("BinaryPackageTester Data files do not fail File type classification");

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

/// Source-package Rule-level test harness: Orig files and Patched files fixtures.
pub struct SourcePackageTester {
    rule: Box<dyn SourceRule>,
    expect_pass: Vec<SourcePackageCase>,
    expect_fail: Vec<SourcePackageCase>,
    snapshot: String,
}

/// One Source package described as Orig files, Patched files, and a package name.
#[derive(Clone, Debug)]
pub struct SourcePackageCase {
    orig: TestCase,
    patched: TestCase,
    package: String,
    dsc: Option<Dsc>,
}

impl SourcePackageCase {
    pub fn orig(mut self, files: impl Into<TestCase>) -> Self {
        self.orig = files.into();
        self
    }

    pub fn dsc(mut self, dsc: Dsc) -> Self {
        self.dsc = Some(dsc);
        self
    }

    #[allow(dead_code)]
    pub fn package(mut self, name: impl Into<String>) -> Self {
        self.package = name.into();
        self
    }

    fn describe(&self) -> String {
        let patched = self.patched.describe();
        let orig = self.orig.describe();
        if orig == "<empty tree>" {
            patched
        } else {
            format!("patched: {patched}; orig: {orig}")
        }
    }
}

impl From<TestCase> for SourcePackageCase {
    fn from(patched: TestCase) -> Self {
        Self {
            orig: TestCase::default(),
            patched,
            package: "example".to_string(),
            dsc: None,
        }
    }
}

impl<P, C> From<Vec<(P, C)>> for SourcePackageCase
where
    P: Into<String>,
    C: Into<TestFile>,
{
    fn from(files: Vec<(P, C)>) -> Self {
        SourcePackageCase::from(TestCase::from(files))
    }
}

impl SourcePackageTester {
    pub fn new<R, P, F, C>(rule: R, expect_pass: P, expect_fail: F) -> Self
    where
        R: SourceRule + 'static,
        P: IntoIterator<Item = C>,
        F: IntoIterator<Item = C>,
        C: Into<SourcePackageCase>,
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
            "{}_{}_source_package",
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

    fn run(&mut self, case: &SourcePackageCase) -> TestResult {
        let data = SourcePackageData::with_file_trees(
            PathBuf::from("example.dsc"),
            case.package.clone(),
            case.orig.file_tree(),
            case.patched.file_tree(),
            case.dsc.clone(),
        );
        let mut ctx = SourceContext::bind_package(&data, self.rule.as_ref());
        self.rule.run(&mut ctx);
        let diagnostics = ctx
            .finish()
            .expect("SourcePackageTester File trees do not call the File type producer");

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
    for (relative, file) in &case.files {
        let path = dir.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|error| panic!("creating {}: {error}", parent.display()));
        }
        fs::write(&path, &file.content)
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
