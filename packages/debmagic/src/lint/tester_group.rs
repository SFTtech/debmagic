//! Rule-level test harnesses for the artifacts of a Group: `ChangesTester`,
//! `BuildinfoTester` and `GroupTester`. They mirror `SourcePackageTester` and
//! `BinaryPackageTester`: pass cases emit no Diagnostics, fail cases at least
//! one, and `test_and_snapshot` records the fail-case Diagnostics with insta.
//! Cases are fixtures, not artifacts on disk, except that a Changes or
//! Buildinfo case may name sibling files, which are written to a temp
//! directory next to the file (a Rule reads the files it names from there).

use std::fmt::Write;
use std::path::PathBuf;

use super::buildinfo::Buildinfo;
use super::changes::Changes;
use super::context::BinaryPackageData;
use super::diagnostic::Diagnostic;
use super::dsc::Dsc;
use super::file_tree::FileTree;
use super::group::{BuildinfoData, ChangesData, DscData, Group};
use super::group_context::{BuildinfoContext, ChangesContext, GroupContext};
use super::installation_control::InstallationControl;
use super::rule::{BuildinfoRule, ChangesRule, GroupRule, RuleAccess};
use super::run::format_diagnostic;
use super::tester::{TestFailure, format_test_failures};

/// Runs one case of a Rule and returns its Diagnostics.
type RunCase<C> = Box<dyn Fn(&C) -> Vec<Diagnostic>>;

/// The shared pass/fail loop.
struct CaseTester<C> {
    code: String,
    tag: String,
    snapshot_suffix: &'static str,
    run: RunCase<C>,
    describe: fn(&C) -> String,
    expect_pass: Vec<C>,
    expect_fail: Vec<C>,
    snapshot: String,
}

impl<C> CaseTester<C> {
    fn new(
        rule: &dyn RuleAccess,
        snapshot_suffix: &'static str,
        run: RunCase<C>,
        describe: fn(&C) -> String,
        expect_pass: Vec<C>,
        expect_fail: Vec<C>,
    ) -> Self {
        Self {
            code: rule.code().to_string(),
            tag: rule.tag().as_str().to_string(),
            snapshot_suffix,
            run,
            describe,
            expect_pass,
            expect_fail,
            snapshot: String::new(),
        }
    }

    fn test(&mut self) {
        let mut failures = Vec::new();
        let mut output_index = 0;
        for case in &self.expect_pass {
            let diagnostics = (self.run)(case);
            for diagnostic in &diagnostics {
                writeln!(self.snapshot, "{}", format_diagnostic(diagnostic))
                    .expect("writing to string never fails");
            }
            if !diagnostics.is_empty() {
                failures.push(TestFailure::ExpectedToPass {
                    case: (self.describe)(case),
                    diagnostic: self.snapshot[output_index..].to_string(),
                });
            }
            output_index = self.snapshot.len();
        }
        let mut unexpected_passes = Vec::new();
        for case in &self.expect_fail {
            let diagnostics = (self.run)(case);
            if diagnostics.is_empty() {
                unexpected_passes.push(TestFailure::ExpectedToFail {
                    case: (self.describe)(case),
                });
            }
            for diagnostic in &diagnostics {
                writeln!(self.snapshot, "{}", format_diagnostic(diagnostic))
                    .expect("writing to string never fails");
            }
        }
        if !failures.is_empty() {
            println!(
                "{}",
                format_test_failures("expected to pass, but failed", &failures)
            );
        }
        if !unexpected_passes.is_empty() {
            println!(
                "{}",
                format_test_failures("expected to fail, but passed", &unexpected_passes)
            );
        }
        assert!(
            failures.is_empty() && unexpected_passes.is_empty(),
            "Some tests failed for Rule {} [{}] (see output above)",
            self.code,
            self.tag,
        );
    }

    /// The snapshot holds the fail cases only, so the pass-case output (empty
    /// when the test passes) does not matter.
    fn test_and_snapshot(&mut self) {
        self.test();
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        settings.set_omit_expression(true);
        let name = format!(
            "{}_{}_{}",
            self.code,
            self.tag.replace('-', "_"),
            self.snapshot_suffix
        );
        settings.bind(|| {
            insta::assert_snapshot!(name, self.snapshot);
        });
    }
}

/// One Changes file as Deb822 text, and the files next to it.
#[derive(Clone, Debug)]
pub struct ChangesCase {
    text: String,
    siblings: Vec<(String, Vec<u8>)>,
}

#[allow(dead_code)]
impl ChangesCase {
    /// A file with a sibling of the given name and content, as the `Files`
    /// field may name.
    pub fn file(mut self, name: impl Into<String>, content: impl Into<Vec<u8>>) -> Self {
        self.siblings.push((name.into(), content.into()));
        self
    }

    fn describe(&self) -> String {
        let mut text = self.text.replace('\n', " | ");
        for (name, _) in &self.siblings {
            write!(text, " + {name}").expect("writing to string never fails");
        }
        text
    }
}

impl<S: Into<String>> From<S> for ChangesCase {
    fn from(text: S) -> Self {
        Self {
            text: text.into(),
            siblings: Vec::new(),
        }
    }
}

/// Changes-file Rule-level test harness: the file as Deb822 text.
pub struct ChangesTester(CaseTester<ChangesCase>);

#[allow(dead_code)]
impl ChangesTester {
    pub fn new<R, P, F, C>(rule: R, expect_pass: P, expect_fail: F) -> Self
    where
        R: ChangesRule + 'static,
        P: IntoIterator<Item = C>,
        F: IntoIterator<Item = C>,
        C: Into<ChangesCase>,
    {
        let rule = std::rc::Rc::new(rule);
        let probe = std::rc::Rc::clone(&rule);
        Self(CaseTester::new(
            rule.as_ref(),
            "changes",
            Box::new(move |case: &ChangesCase| {
                let dir = tempfile::tempdir().expect("creating temp Changes directory");
                for (name, content) in &case.siblings {
                    std::fs::write(dir.path().join(name), content)
                        .unwrap_or_else(|error| panic!("writing {name}: {error}"));
                }
                let changes = Changes::parse(&case.text).expect("ChangesCase text is Deb822");
                let data = ChangesData {
                    path: dir.path().join("test.changes"),
                    package_name: changes.source().unwrap_or("example").to_string(),
                    changes,
                };
                let mut ctx = ChangesContext::bind(&data, probe.as_ref());
                ChangesRule::run(probe.as_ref(), &mut ctx);
                ctx.finish()
            }),
            ChangesCase::describe,
            expect_pass.into_iter().map(Into::into).collect(),
            expect_fail.into_iter().map(Into::into).collect(),
        ))
    }

    pub fn test(&mut self) {
        self.0.test();
    }

    pub fn test_and_snapshot(&mut self) {
        self.0.test_and_snapshot();
    }
}

/// One Buildinfo file as Deb822 text.
#[derive(Clone, Debug)]
pub struct BuildinfoCase {
    text: String,
}

impl<S: Into<String>> From<S> for BuildinfoCase {
    fn from(text: S) -> Self {
        Self { text: text.into() }
    }
}

/// Buildinfo-file Rule-level test harness: the file as Deb822 text.
pub struct BuildinfoTester(CaseTester<BuildinfoCase>);

#[allow(dead_code)]
impl BuildinfoTester {
    pub fn new<R, P, F, C>(rule: R, expect_pass: P, expect_fail: F) -> Self
    where
        R: BuildinfoRule + 'static,
        P: IntoIterator<Item = C>,
        F: IntoIterator<Item = C>,
        C: Into<BuildinfoCase>,
    {
        let rule = std::rc::Rc::new(rule);
        let probe = std::rc::Rc::clone(&rule);
        Self(CaseTester::new(
            rule.as_ref(),
            "buildinfo",
            Box::new(move |case: &BuildinfoCase| {
                let buildinfo = Buildinfo::parse(&case.text).expect("BuildinfoCase text is Deb822");
                let data = BuildinfoData {
                    path: PathBuf::from("test.buildinfo"),
                    package_name: buildinfo.source().unwrap_or("example").to_string(),
                    buildinfo,
                };
                let mut ctx = BuildinfoContext::bind(&data, probe.as_ref());
                BuildinfoRule::run(probe.as_ref(), &mut ctx);
                ctx.finish()
            }),
            |case| case.text.replace('\n', " | "),
            expect_pass.into_iter().map(Into::into).collect(),
            expect_fail.into_iter().map(Into::into).collect(),
        ))
    }

    pub fn test(&mut self) {
        self.0.test();
    }

    pub fn test_and_snapshot(&mut self) {
        self.0.test_and_snapshot();
    }
}

/// One Group described by its artifacts: any of a Changes file, a Dsc and a
/// Buildinfo file as Deb822 text, and installables by name and Installation
/// control fields. The source package is `example` at `1.0` unless set.
#[derive(Clone, Debug)]
pub struct GroupCase {
    source: String,
    version: String,
    changes: Option<String>,
    dsc: Option<String>,
    buildinfo: Option<String>,
    installables: Vec<(String, String, bool)>,
}

impl Default for GroupCase {
    fn default() -> Self {
        Self {
            source: "example".to_string(),
            version: "1.0".to_string(),
            changes: None,
            dsc: None,
            buildinfo: None,
            installables: Vec::new(),
        }
    }
}

#[allow(dead_code)]
impl GroupCase {
    /// A Group with a `.dsc` (Group Rules run only on a Group that has one).
    /// `dsc` is the Deb822 text of the paragraph.
    pub fn new(dsc: impl Into<String>) -> Self {
        Self {
            dsc: Some(dsc.into()),
            ..Self::default()
        }
    }

    /// The source package name Diagnostics carry.
    pub fn source(mut self, name: impl Into<String>) -> Self {
        self.source = name.into();
        self
    }

    pub fn changes(mut self, text: impl Into<String>) -> Self {
        self.changes = Some(text.into());
        self
    }

    pub fn buildinfo(mut self, text: impl Into<String>) -> Self {
        self.buildinfo = Some(text.into());
        self
    }

    /// A deb of the Group. `fields` is more Installation control, as Deb822
    /// lines (`Depends: other\n`); `Package`, `Version` and `Architecture` are
    /// set.
    pub fn installable(mut self, name: impl Into<String>, fields: impl Into<String>) -> Self {
        self.installables.push((name.into(), fields.into(), false));
        self
    }

    /// A udeb of the Group.
    pub fn udeb(mut self, name: impl Into<String>, fields: impl Into<String>) -> Self {
        self.installables.push((name.into(), fields.into(), true));
        self
    }

    fn describe(&self) -> String {
        let installables: Vec<String> = self
            .installables
            .iter()
            .map(|(name, fields, udeb)| {
                format!(
                    "{}{name} [{}]",
                    if *udeb { "udeb " } else { "" },
                    fields.trim().replace('\n', "; ")
                )
            })
            .collect();
        format!(
            "{}_{}: {}",
            self.source,
            self.version,
            installables.join(", ")
        )
    }

    fn group(&self) -> Group {
        let changes = self.changes.as_ref().map(|text| {
            let changes = Changes::parse(text).expect("GroupCase changes is Deb822");
            ChangesData {
                path: PathBuf::from("test.changes"),
                package_name: self.source.clone(),
                changes,
            }
        });
        let source = self.dsc.as_ref().map(|text| DscData {
            path: PathBuf::from("example.dsc"),
            dsc: Dsc::parse(text).expect("GroupCase dsc is Deb822"),
        });
        let buildinfo = self.buildinfo.as_ref().map(|text| BuildinfoData {
            path: PathBuf::from("test.buildinfo"),
            package_name: self.source.clone(),
            buildinfo: Buildinfo::parse(text).expect("GroupCase buildinfo is Deb822"),
        });
        let mut binaries: Vec<BinaryPackageData> = self
            .installables
            .iter()
            .map(|(name, fields, udeb)| {
                let mut entries = vec![
                    ("Package".to_string(), name.clone()),
                    ("Version".to_string(), self.version.clone()),
                    ("Architecture".to_string(), "all".to_string()),
                    ("Source".to_string(), self.source.clone()),
                ];
                for line in fields.lines().filter(|line| !line.trim().is_empty()) {
                    let (key, value) = line
                        .split_once(':')
                        .expect("fields are `Name: value` lines");
                    entries.push((key.trim().to_string(), value.trim().to_string()));
                }
                let extension = if *udeb { "udeb" } else { "deb" };
                BinaryPackageData::with_data_files(
                    PathBuf::from(format!("{name}_{}_all.{extension}", self.version)),
                    name.clone(),
                    InstallationControl::from_fields(entries),
                    FileTree::from_entries(None, Vec::new()),
                )
            })
            .collect();
        // Group assembly orders installables: debs first, then udebs.
        binaries.sort_by_key(|binary| binary.path().extension().is_some_and(|ext| ext == "udeb"));
        Group::fixture(
            &self.source,
            &self.version,
            changes,
            source,
            buildinfo,
            binaries,
        )
    }
}

impl From<&str> for GroupCase {
    /// A Group of one `.dsc`, given as Deb822 text.
    fn from(dsc: &str) -> Self {
        Self::new(dsc)
    }
}

/// Group Rule-level test harness: a Group of fixture artifacts.
pub struct GroupTester(CaseTester<GroupCase>);

#[allow(dead_code)]
impl GroupTester {
    pub fn new<R, P, F, C>(rule: R, expect_pass: P, expect_fail: F) -> Self
    where
        R: GroupRule + 'static,
        P: IntoIterator<Item = C>,
        F: IntoIterator<Item = C>,
        C: Into<GroupCase>,
    {
        let rule = std::rc::Rc::new(rule);
        let probe = std::rc::Rc::clone(&rule);
        Self(CaseTester::new(
            rule.as_ref(),
            "group",
            Box::new(move |case: &GroupCase| {
                let group = case.group();
                let mut ctx = GroupContext::bind(&group, probe.as_ref());
                GroupRule::run(probe.as_ref(), &mut ctx);
                ctx.finish()
            }),
            GroupCase::describe,
            expect_pass.into_iter().map(Into::into).collect(),
            expect_fail.into_iter().map(Into::into).collect(),
        ))
    }

    pub fn test(&mut self) {
        self.0.test();
    }

    pub fn test_and_snapshot(&mut self) {
        self.0.test_and_snapshot();
    }
}
