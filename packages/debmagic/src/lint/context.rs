use std::cell::OnceCell;
use std::path::{Path, PathBuf};

use super::binary_package_reader::BinaryPackageReader;
use super::debian_changelog::DebianChangelog;
use super::debian_control::DebianControl;
use super::debian_rules::DebianRules;
use super::diagnostic::{Diagnostic, ProcessableType};
use super::installation_control::InstallationControl;
use super::rule::{Code, RuleAccess, Tag};

struct DiagnosticSink {
    code: Code,
    tag: Tag,
    processable_type: ProcessableType,
    package: OnceCell<String>,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticSink {
    fn new(rule: &(impl RuleAccess + ?Sized), processable_type: ProcessableType) -> Self {
        Self {
            code: rule.code(),
            tag: rule.tag(),
            processable_type,
            package: OnceCell::new(),
            diagnostics: Vec::new(),
        }
    }

    fn report(&mut self, mut diagnostic: Diagnostic, resolve_package: impl FnOnce() -> String) {
        diagnostic.code = self.code;
        diagnostic.tag = self.tag;
        if diagnostic.package.is_empty() {
            if self.package.get().is_none() {
                let _ = self.package.set(resolve_package());
            }
            diagnostic.package = self
                .package
                .get()
                .expect("package name is initialized")
                .clone();
        }
        if diagnostic.processable_type.is_none() {
            diagnostic.processable_type = Some(self.processable_type);
        }
        self.diagnostics.push(diagnostic);
    }

    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

pub(crate) struct SourceTreeData {
    root: PathBuf,
    debian_control: OnceCell<Option<DebianControl>>,
    debian_changelog: OnceCell<Option<DebianChangelog>>,
    debian_rules: OnceCell<Option<DebianRules>>,
}

impl SourceTreeData {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            debian_control: OnceCell::new(),
            debian_changelog: OnceCell::new(),
            debian_rules: OnceCell::new(),
        }
    }

    fn debian_control(&self) -> Option<&DebianControl> {
        self.debian_control
            .get_or_init(|| DebianControl::load(&self.root.join("debian/control")))
            .as_ref()
    }

    fn debian_changelog(&self) -> Option<&DebianChangelog> {
        self.debian_changelog
            .get_or_init(|| DebianChangelog::load(&self.root.join("debian/changelog")))
            .as_ref()
    }

    fn debian_rules(&self) -> Option<&DebianRules> {
        self.debian_rules
            .get_or_init(|| DebianRules::load(&self.root.join("debian/rules")))
            .as_ref()
    }

    fn package_name(&self) -> String {
        if let Some(name) = self
            .debian_control()
            .and_then(|control| control.source().get("Source"))
            .filter(|name| !name.is_empty())
        {
            return name.to_string();
        }
        changelog_package_name(&self.root).unwrap_or_default()
    }
}

pub(crate) struct SourceTreeContext<'a> {
    data: &'a SourceTreeData,
    sink: DiagnosticSink,
}

impl<'a> SourceTreeContext<'a> {
    pub fn bind(data: &'a SourceTreeData, rule: &(impl RuleAccess + ?Sized)) -> Self {
        Self {
            data,
            sink: DiagnosticSink::new(rule, ProcessableType::Source),
        }
    }

    pub fn path(&self) -> &Path {
        &self.data.root
    }

    /// Structured `debian/control`. `None` if the file has a Deb822 syntax error.
    pub fn debian_control(&self) -> Option<&DebianControl> {
        self.data.debian_control()
    }

    /// Parsed `debian/changelog`. `None` if the file is missing, unreadable, or not UTF-8.
    pub fn debian_changelog(&self) -> Option<&DebianChangelog> {
        self.data.debian_changelog()
    }

    /// Parsed `debian/rules`. `None` if the file is missing, unreadable, or not UTF-8.
    /// Follows a symlink.
    pub fn debian_rules(&self) -> Option<&DebianRules> {
        self.data.debian_rules()
    }

    /// Report a finding. Code and Tag come from the running Rule; Severity is
    /// set on the Diagnostic (and may be remapped later from config). Package
    /// and processable type default from this Subject when the Rule omits them.
    pub fn diagnostic(&mut self, diagnostic: Diagnostic) {
        let data = self.data;
        self.sink.report(diagnostic, || data.package_name());
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.sink.into_diagnostics()
    }
}

pub(crate) struct BinaryPackageData {
    reader: BinaryPackageReader,
    installation_control: InstallationControl,
}

impl BinaryPackageData {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let reader = BinaryPackageReader::from_path(path)?;
        let installation_control = InstallationControl::from_reader(&reader)?;
        Ok(Self {
            reader,
            installation_control,
        })
    }

    fn package_name(&self) -> String {
        self.installation_control
            .fields()
            .get("Package")
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| super::binary_package_reader::guess_package_name(self.reader.path()))
    }
}

pub(crate) struct BinaryPackageContext<'a> {
    data: &'a BinaryPackageData,
    sink: DiagnosticSink,
}

impl<'a> BinaryPackageContext<'a> {
    pub fn bind(data: &'a BinaryPackageData, rule: &(impl RuleAccess + ?Sized)) -> Self {
        Self {
            data,
            sink: DiagnosticSink::new(rule, processable_type_for_binary(data.reader.path())),
        }
    }

    pub fn path(&self) -> &Path {
        self.data.reader.path()
    }

    pub fn installation_control(&self) -> &InstallationControl {
        &self.data.installation_control
    }

    #[allow(dead_code)]
    pub fn reader(&self) -> &BinaryPackageReader {
        &self.data.reader
    }

    /// Report a finding. Code and Tag come from the running Rule; Severity is
    /// set on the Diagnostic (and may be remapped later from config). Package
    /// and processable type default from this Subject when the Rule omits them.
    pub fn diagnostic(&mut self, diagnostic: Diagnostic) {
        let data = self.data;
        self.sink.report(diagnostic, || data.package_name());
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.sink.into_diagnostics()
    }
}

#[allow(dead_code)]
pub(crate) struct SourcePackageData {
    path: PathBuf,
}

#[allow(dead_code)]
impl SourcePackageData {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[allow(dead_code)]
pub(crate) struct SourcePackageContext<'a> {
    data: &'a SourcePackageData,
    sink: DiagnosticSink,
}

#[allow(dead_code)]
impl<'a> SourcePackageContext<'a> {
    pub fn bind(data: &'a SourcePackageData, rule: &(impl RuleAccess + ?Sized)) -> Self {
        Self {
            data,
            sink: DiagnosticSink::new(rule, ProcessableType::Source),
        }
    }

    pub fn path(&self) -> &Path {
        &self.data.path
    }

    pub fn diagnostic(&mut self, diagnostic: Diagnostic) {
        self.sink.report(diagnostic, String::new);
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.sink.into_diagnostics()
    }
}

fn processable_type_for_binary(path: &Path) -> ProcessableType {
    if path.extension().and_then(|ext| ext.to_str()) == Some("udeb") {
        ProcessableType::Udeb
    } else {
        ProcessableType::Binary
    }
}

fn changelog_package_name(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("debian/changelog")).ok()?;
    let changelog: ::debian_changelog::ChangeLog = text.parse().ok()?;
    changelog.into_iter().next()?.package()
}
