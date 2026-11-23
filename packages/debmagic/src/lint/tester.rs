use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use super::context::{
    BinaryPackageContext, BinaryPackageData, SourceContext, SourcePackageData, SourceTreeData,
};
use super::deb_members::DebMembers;
use super::diagnostic::Diagnostic;
use super::dsc::Dsc;
use super::file_tree::{Entry, EntryKind, FileTree, Owner};
use super::file_type::FileType;
use super::installation_control::InstallationControl;
use super::rule::{BinaryPackageRule, SourceRule};
use super::run::format_diagnostic;
#[allow(unused_imports)]
pub use super::tester_group::{
    BuildinfoCase, BuildinfoTester, ChangesCase, ChangesTester, GroupCase, GroupTester,
};

/// One Source tree described as a map of relative paths to file contents.
#[derive(Clone, Debug, Default)]
pub struct TestCase {
    /// Keys are raw paths, so a case can hold a name that is not UTF-8.
    files: BTreeMap<OsString, TestFile>,
}

/// One fixture Entry. Mirrors [`Entry`]: only the data a variant has.
///
/// `mode` is `None` for the usual mode of the kind (see [`Entry::with_mode`]).
/// `owner` is `None` for no say: a Source tree fixture then has no owner, and
/// a Binary package fixture gets `root/root`, as `dpkg-deb` would give it.
#[derive(Clone, Debug)]
pub enum TestFile {
    RegularFile {
        content: Vec<u8>,
        file_type: Option<FileType>,
        mode: Option<u32>,
        owner: Option<Owner>,
    },
    /// `target` is the linked member path, `None` when the case does not
    /// care (as on a host File tree).
    Hardlink {
        content: Vec<u8>,
        file_type: Option<FileType>,
        mode: Option<u32>,
        owner: Option<Owner>,
        target: Option<PathBuf>,
    },
    Directory {
        mode: Option<u32>,
        owner: Option<Owner>,
    },
    /// `target` is exactly as stored in the package.
    Symlink {
        target: PathBuf,
        mode: Option<u32>,
        owner: Option<Owner>,
    },
    /// A named pipe, a character device or a block device.
    Special {
        kind: EntryKind,
        mode: Option<u32>,
        owner: Option<Owner>,
    },
}

impl TestFile {
    /// A regular file with UTF-8 `content` and no File type.
    pub fn new(content: impl Into<String>) -> Self {
        Self::from_bytes(content.into().into_bytes())
    }

    /// A regular file with raw `content` and no File type.
    pub fn from_bytes(content: impl Into<Vec<u8>>) -> Self {
        Self::RegularFile {
            content: content.into(),
            file_type: None,
            mode: None,
            owner: None,
        }
    }

    pub fn directory() -> Self {
        Self::Directory {
            mode: None,
            owner: None,
        }
    }

    pub fn symlink(target: impl Into<PathBuf>) -> Self {
        Self::Symlink {
            target: target.into(),
            mode: None,
            owner: None,
        }
    }

    /// A named pipe.
    pub fn fifo() -> Self {
        Self::special(EntryKind::Fifo)
    }

    /// A character device node.
    #[allow(dead_code)]
    pub fn char_device() -> Self {
        Self::special(EntryKind::CharDevice)
    }

    /// A block device node.
    #[allow(dead_code)]
    pub fn block_device() -> Self {
        Self::special(EntryKind::BlockDevice)
    }

    fn special(kind: EntryKind) -> Self {
        Self::Special {
            kind,
            mode: None,
            owner: None,
        }
    }

    /// Make a regular file a hardlink. Panics on other variants.
    pub fn hardlink(self) -> Self {
        match self {
            Self::RegularFile {
                content,
                file_type,
                mode,
                owner,
            } => Self::Hardlink {
                content,
                file_type,
                mode,
                owner,
                target: None,
            },
            other => panic!("only a regular file can become a hardlink, not {other:?}"),
        }
    }

    /// Make a regular file a hardlink to the member at `target` (normalized,
    /// as in the tar header). Panics on other variants.
    pub fn hardlink_to(self, target: impl Into<PathBuf>) -> Self {
        match self.hardlink() {
            Self::Hardlink {
                content,
                file_type,
                mode,
                owner,
                ..
            } => Self::Hardlink {
                content,
                file_type,
                mode,
                owner,
                target: Some(target.into()),
            },
            _ => unreachable!("hardlink() returns a hardlink"),
        }
    }

    /// Set the File type of a regular file or hardlink. Panics on other variants.
    pub fn file_type(mut self, new: FileType) -> Self {
        match &mut self {
            Self::RegularFile { file_type, .. } | Self::Hardlink { file_type, .. } => {
                *file_type = Some(new);
            }
            other => panic!("only a regular file or hardlink has a File type, not {other:?}"),
        }
        self
    }

    /// Set the permission bits (`0o7777`), as a tar header stores them.
    pub fn mode(mut self, new: u32) -> Self {
        match &mut self {
            Self::RegularFile { mode, .. }
            | Self::Hardlink { mode, .. }
            | Self::Directory { mode, .. }
            | Self::Symlink { mode, .. }
            | Self::Special { mode, .. } => *mode = Some(new),
        }
        self
    }

    /// Set the owner by name, as a tar header stores it: `root` has id `0`,
    /// any other name has id `1000`. Without this, a Binary package fixture
    /// is owned by `root/root` and a Source tree fixture has no owner.
    pub fn owned_by(self, user: &str, group: &str) -> Self {
        let id = |name: &str| if name == "root" { 0 } else { 1000 };
        self.owner(Owner::named(id(user), id(group), user, group))
    }

    /// Set the full [`Owner`].
    pub fn owner(mut self, new: Owner) -> Self {
        match &mut self {
            Self::RegularFile { owner, .. }
            | Self::Hardlink { owner, .. }
            | Self::Directory { owner, .. }
            | Self::Symlink { owner, .. }
            | Self::Special { owner, .. } => *owner = Some(new),
        }
        self
    }

    /// The Entry at `path`. `default_owner` is given to an Entry that has no
    /// owner set.
    fn entry(&self, path: &OsStr, default_owner: Option<&Owner>) -> Entry {
        let (entry, mode, owner) = match self {
            Self::RegularFile {
                content,
                file_type,
                mode,
                owner,
            } => (
                Entry::regular_file(path, *file_type, Some(content.clone())),
                mode,
                owner,
            ),
            Self::Hardlink {
                content,
                file_type,
                mode,
                owner,
                target,
            } => {
                let entry = Entry::hardlink(path, *file_type, Some(content.clone()));
                match target {
                    Some(target) => (entry.with_hardlink_target(target), mode, owner),
                    None => (entry, mode, owner),
                }
            }
            Self::Directory { mode, owner } => (Entry::directory(path), mode, owner),
            Self::Symlink {
                target,
                mode,
                owner,
            } => (Entry::symlink(path, target), mode, owner),
            Self::Special { kind, mode, owner } => (
                match kind {
                    EntryKind::Fifo => Entry::fifo(path),
                    EntryKind::CharDevice => Entry::char_device(path),
                    EntryKind::BlockDevice => Entry::block_device(path),
                    other => unreachable!("{other:?} is not a special file"),
                },
                mode,
                owner,
            ),
        };
        let entry = match mode {
            Some(mode) => entry.with_mode(*mode),
            None => entry,
        };
        match owner.as_ref().or(default_owner) {
            Some(owner) => entry.with_owner(owner.clone()),
            None => entry,
        }
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
    /// The names of the top-level files of the case, as UTF-8 text.
    fn file_names(&self) -> Vec<String> {
        self.files
            .keys()
            .filter_map(|name| name.to_str().map(str::to_string))
            .collect()
    }

    pub fn source_tree<P, C>(files: impl IntoIterator<Item = (P, C)>) -> Self
    where
        P: Into<OsString>,
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
        self.files
            .keys()
            .map(|path| path.to_string_lossy())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// A File tree with no owner on Entries that fixtures give none.
    pub(super) fn file_tree(&self) -> FileTree {
        self.file_tree_owned_by(None)
    }

    /// A File tree where Entries that fixtures give no owner are owned by
    /// `root/root`, as in a Binary package built by `dpkg-deb`.
    pub(super) fn package_file_tree(&self) -> FileTree {
        self.file_tree_owned_by(Some(&Owner::root()))
    }

    fn file_tree_owned_by(&self, default_owner: Option<&Owner>) -> FileTree {
        let entries = self
            .files
            .iter()
            .map(|(path, file)| file.entry(path, default_owner))
            .collect();
        FileTree::from_entries(None, entries)
    }
}

impl<P, C> From<Vec<(P, C)>> for TestCase
where
    P: Into<OsString>,
    C: Into<TestFile>,
{
    fn from(files: Vec<(P, C)>) -> Self {
        Self::source_tree(files)
    }
}

impl<const N: usize, P, C> From<[(P, C); N]> for TestCase
where
    P: Into<OsString>,
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
    control_files: TestCase,
    package: String,
    fields: Vec<(String, String)>,
    udeb: bool,
    deb_members: Option<Vec<String>>,
}

impl BinaryPackageCase {
    pub fn package(mut self, name: impl Into<String>) -> Self {
        self.package = name.into();
        self
    }

    /// Set an Installation control field (the package name is `package`).
    pub fn field(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push((name.into(), value.into()));
        self
    }

    /// Add a Control file (`postinst`, `md5sums`, ...) with `file`'s content.
    /// Every case has Control files, empty unless set.
    pub fn control_file(mut self, name: impl Into<OsString>, file: impl Into<TestFile>) -> Self {
        self.control_files.files.insert(name.into(), file.into());
        self
    }

    /// Set the ar members of the `.deb`, in archive order. Without this a case
    /// has the canonical `debian-binary`, `control.tar.xz`, `data.tar.xz`.
    pub fn deb_members<S: Into<String>>(mut self, names: impl IntoIterator<Item = S>) -> Self {
        self.deb_members = Some(names.into_iter().map(Into::into).collect());
        self
    }

    /// Make the Binary package a udeb.
    pub fn udeb(mut self) -> Self {
        self.udeb = true;
        self
    }

    fn describe(&self) -> String {
        let files = self.files.describe();
        let mut prefix = String::new();
        if self.package != "example" {
            prefix.push_str(&format!("package {}: ", self.package));
        }
        if self.udeb {
            prefix.push_str("udeb: ");
        }
        if let Some(members) = &self.deb_members {
            prefix.push_str(&format!("members {}: ", members.join(" ")));
        }
        for (name, value) in &self.fields {
            prefix.push_str(&format!("{name}={value:?}: "));
        }
        for (name, file) in &self.control_files.files {
            prefix.push_str(&format!("control {}={file:?}: ", name.display()));
        }
        format!("{prefix}{files}")
    }
}

impl From<TestCase> for BinaryPackageCase {
    fn from(files: TestCase) -> Self {
        Self {
            files,
            control_files: TestCase::default(),
            package: "example".to_string(),
            fields: Vec::new(),
            udeb: false,
            deb_members: None,
        }
    }
}

impl<P, C> From<Vec<(P, C)>> for BinaryPackageCase
where
    P: Into<OsString>,
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
        self.test_and_snapshot_with_suffix("");
    }

    /// Snapshot as `{code}_{tag}_{suffix}` so a Rule that also snapshots
    /// Source-tree cases keeps a separate Binary-package oracle.
    pub fn test_and_snapshot_with_suffix(&mut self, suffix: &str) {
        self.test();
        self.snapshot(suffix);
    }

    fn snapshot(&self, suffix: &str) {
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        settings.set_omit_expression(true);
        let mut name = format!(
            "{}_{}",
            self.rule.code(),
            self.rule.tag().as_str().replace('-', "_")
        );
        if !suffix.is_empty() {
            name.push('_');
            name.push_str(suffix);
        }
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
        let mut fields = vec![("Package".to_string(), case.package.clone())];
        fields.extend(case.fields.iter().cloned());
        let data = BinaryPackageData::with_data_files(
            PathBuf::from(if case.udeb {
                "example.udeb"
            } else {
                "example.deb"
            }),
            case.package.clone(),
            InstallationControl::from_fields(fields),
            case.files.package_file_tree(),
        )
        .with_control_files(case.control_files.package_file_tree())
        .with_deb_members(match &case.deb_members {
            Some(names) => DebMembers::from_names(names.iter().map(String::as_str)),
            None => DebMembers::canonical(),
        });
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
    /// Files next to the `.dsc` (its directory): signatures, component tarballs.
    siblings: TestCase,
    package: String,
    dsc: Option<Dsc>,
}

impl SourcePackageCase {
    pub fn orig(mut self, files: impl Into<TestCase>) -> Self {
        self.orig = files.into();
        self
    }

    /// Files beside the `.dsc`, beside the orig and Patched files: the `.dsc`
    /// lists them in its `Files` field (see `SourceContext::sibling_files`).
    /// Without a `.dsc` case, they are all listed.
    pub fn siblings(mut self, files: impl Into<TestCase>) -> Self {
        self.siblings = files.into();
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
            siblings: TestCase::default(),
            package: "example".to_string(),
            dsc: None,
        }
    }
}

impl<P, C> From<Vec<(P, C)>> for SourcePackageCase
where
    P: Into<OsString>,
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
        let siblings = materialize(&case.siblings);
        // A case with siblings but no `.dsc` lists them all in its `Files`.
        let dsc = match &case.dsc {
            Some(dsc) => Some(dsc.clone()),
            None if !case.siblings.files.is_empty() => {
                Some(listing_dsc(&case.package, &case.siblings.file_names()))
            }
            None => None,
        };
        let data = SourcePackageData::with_file_trees(
            siblings.path().join("example.dsc"),
            case.package.clone(),
            case.orig.file_tree(),
            case.patched.file_tree(),
            dsc,
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

/// A `.dsc` of version 1.0 whose `Files` field lists `names`, for a Source
/// package case that gives siblings and no `.dsc`.
fn listing_dsc(package: &str, names: &[String]) -> Dsc {
    let mut text = format!("Source: {package}\nVersion: 1.0\nFiles:\n");
    for name in names {
        text.push_str(&format!(" 00000000000000000000000000000000 0 {name}\n"));
    }
    Dsc::parse(&text).expect("generated .dsc paragraph")
}

pub(super) enum TestFailure {
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
        match file {
            TestFile::RegularFile { content, .. } | TestFile::Hardlink { content, .. } => {
                fs::write(&path, content)
                    .unwrap_or_else(|error| panic!("writing {}: {error}", path.display()));
            }
            TestFile::Directory { .. } => fs::create_dir_all(&path)
                .unwrap_or_else(|error| panic!("creating {}: {error}", path.display())),
            TestFile::Symlink { target, .. } => std::os::unix::fs::symlink(target, &path)
                .unwrap_or_else(|error| panic!("linking {}: {error}", path.display())),
            TestFile::Special { kind, .. } => {
                assert!(
                    *kind == EntryKind::Fifo,
                    "a Source tree fixture can only hold a fifo, not {kind:?}"
                );
                let status = std::process::Command::new("mkfifo")
                    .arg(&path)
                    .status()
                    .unwrap_or_else(|error| panic!("running mkfifo: {error}"));
                assert!(status.success(), "mkfifo {} failed", path.display());
            }
        }
    }
    dir
}

fn relativize_location(diagnostic: &mut Diagnostic, root: &Path) {
    if let Ok(relative) = diagnostic.location.path.strip_prefix(root) {
        diagnostic.location.path = PathBuf::from(relative);
    }
}

pub(super) fn format_test_failures(reason: &str, failures: &[TestFailure]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_fixtures_default_to_root_and_source_fixtures_to_no_owner() {
        let case = TestCase::source_tree([
            ("a", TestFile::new("x")),
            ("b", TestFile::new("x").owned_by("root", "games")),
            ("pipe", TestFile::fifo()),
        ]);
        let identity = |tree: &FileTree, name: &str| {
            tree.get(name).and_then(Entry::owner).map(Owner::identity)
        };
        let package = case.package_file_tree();
        assert_eq!(identity(&package, "a").as_deref(), Some("root/root"));
        assert_eq!(identity(&package, "b").as_deref(), Some("root/games"));
        assert_eq!(identity(&package, "pipe").as_deref(), Some("root/root"));
        assert_eq!(package.get("pipe").map(Entry::kind), Some(EntryKind::Fifo));
        let source = case.file_tree();
        assert_eq!(identity(&source, "a"), None);
        assert_eq!(identity(&source, "b").as_deref(), Some("root/games"));
        assert_eq!(identity(&source, "pipe"), None);
    }

    #[test]
    fn source_tree_fixture_materializes_a_fifo() {
        let case = TestCase::source_tree([("sub/pipe", TestFile::fifo())]);
        let dir = materialize(&case);
        let tree = FileTree::from_host_directory(dir.path()).expect("walk");
        assert_eq!(tree.get("sub/pipe").map(Entry::kind), Some(EntryKind::Fifo));
    }
}
