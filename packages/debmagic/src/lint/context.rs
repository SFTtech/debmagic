use std::cell::OnceCell;
use std::path::{Path, PathBuf};

use anyhow::anyhow;

use super::binary_package_reader::BinaryPackageReader;
use super::debian_changelog::DebianChangelog;
use super::debian_control::DebianControl;
use super::diagnostic::{Diagnostic, ProcessableType};
use super::dsc::Dsc;
use super::file_tree::FileTree;
use super::installation_control::InstallationControl;
use super::rule::{Code, RuleAccess, Tag};
use super::source_package::SourcePackageExtract;
use super::subject::SubjectKind;

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
    file_tree: OnceCell<Result<FileTree, String>>,
}

impl SourceTreeData {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            debian_control: OnceCell::new(),
            debian_changelog: OnceCell::new(),
            file_tree: OnceCell::new(),
        }
    }

    #[cfg(test)]
    pub fn with_file_tree(root: PathBuf, file_tree: FileTree) -> Self {
        let cell = OnceCell::new();
        let _ = cell.set(Ok(file_tree));
        Self {
            root,
            debian_control: OnceCell::new(),
            debian_changelog: OnceCell::new(),
            file_tree: cell,
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

    fn file_tree(&self) -> Result<&FileTree, &str> {
        match self.file_tree.get_or_init(|| {
            FileTree::from_host_directory(&self.root).map_err(|error| error.to_string())
        }) {
            Ok(tree) => Ok(tree),
            Err(message) => Err(message.as_str()),
        }
    }
}

pub(crate) struct BinaryPackageData {
    path: PathBuf,
    package_name: String,
    reader: Option<BinaryPackageReader>,
    installation_control: InstallationControl,
    data_files: OnceCell<Result<FileTree, String>>,
}

impl BinaryPackageData {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let reader = BinaryPackageReader::from_path(path.clone())?;
        let installation_control = InstallationControl::from_reader(&reader)?;
        let package_name = installation_control
            .fields()
            .get("Package")
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| super::binary_package_reader::guess_package_name(reader.path()));
        Ok(Self {
            path,
            package_name,
            reader: Some(reader),
            installation_control,
            data_files: OnceCell::new(),
        })
    }

    #[cfg(test)]
    pub fn with_data_files(path: PathBuf, package_name: String, data_files: FileTree) -> Self {
        let cell = OnceCell::new();
        let _ = cell.set(Ok(data_files));
        Self {
            path,
            package_name,
            reader: None,
            installation_control: InstallationControl::default(),
            data_files: cell,
        }
    }

    fn data_files(&self) -> Result<&FileTree, &str> {
        match self.data_files.get_or_init(|| match &self.reader {
            Some(reader) => reader.data_file_tree().map_err(|error| error.to_string()),
            None => Err("Binary package Data files are missing".to_string()),
        }) {
            Ok(tree) => Ok(tree),
            Err(message) => Err(message.as_str()),
        }
    }
}

pub(crate) struct BinaryPackageContext<'a> {
    data: &'a BinaryPackageData,
    sink: DiagnosticSink,
    runtime_error: Option<anyhow::Error>,
}

impl<'a> BinaryPackageContext<'a> {
    pub fn bind(data: &'a BinaryPackageData, rule: &(impl RuleAccess + ?Sized)) -> Self {
        Self {
            data,
            sink: DiagnosticSink::new(rule, processable_type_for_binary(&data.path)),
            runtime_error: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.data.path
    }

    pub fn package_name(&self) -> &str {
        &self.data.package_name
    }

    pub fn installation_control(&self) -> &InstallationControl {
        &self.data.installation_control
    }

    /// Data files File tree. `None` after a global File tree failure; that
    /// failure is returned from [`Self::finish`].
    pub fn data_files(&mut self) -> Option<&FileTree> {
        match self.data.data_files() {
            Ok(tree) => Some(tree),
            Err(message) => {
                if self.runtime_error.is_none() {
                    self.runtime_error = Some(anyhow!("{message}"));
                }
                None
            }
        }
    }

    /// Report a finding. Code and Tag come from the running Rule; Severity is
    /// set on the Diagnostic (and may be remapped later from config). Package
    /// and processable type default from this Subject when the Rule omits them.
    pub fn diagnostic(&mut self, diagnostic: Diagnostic) {
        let data = self.data;
        self.sink.report(diagnostic, || data.package_name.clone());
    }

    pub fn finish(self) -> anyhow::Result<Vec<Diagnostic>> {
        if let Some(error) = self.runtime_error {
            return Err(error);
        }
        Ok(self.sink.into_diagnostics())
    }
}

pub(crate) struct SourcePackageData {
    path: PathBuf,
    package_name: String,
    dsc: Option<Dsc>,
    debian_control: OnceCell<Option<DebianControl>>,
    debian_changelog: OnceCell<Option<DebianChangelog>>,
    orig: OnceCell<Result<FileTree, String>>,
    patched: OnceCell<Result<FileTree, String>>,
    _extract: Option<SourcePackageExtract>,
}

impl SourcePackageData {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let extract = SourcePackageExtract::from_dsc(path.clone())?;
        Ok(Self {
            path,
            package_name: extract.package_name.clone(),
            dsc: Some(extract.dsc.clone()),
            debian_control: OnceCell::new(),
            debian_changelog: OnceCell::new(),
            orig: OnceCell::new(),
            patched: OnceCell::new(),
            _extract: Some(extract),
        })
    }

    #[cfg(test)]
    pub fn with_file_trees(
        path: PathBuf,
        package_name: String,
        orig: FileTree,
        patched: FileTree,
        dsc: Option<Dsc>,
    ) -> Self {
        let orig_cell = OnceCell::new();
        let _ = orig_cell.set(Ok(orig));
        let patched_cell = OnceCell::new();
        let _ = patched_cell.set(Ok(patched));
        Self {
            path,
            package_name,
            dsc,
            debian_control: OnceCell::new(),
            debian_changelog: OnceCell::new(),
            orig: orig_cell,
            patched: patched_cell,
            _extract: None,
        }
    }

    fn debian_control(&self) -> Option<&DebianControl> {
        self.debian_control
            .get_or_init(|| match &self._extract {
                Some(extract) => {
                    DebianControl::load(&extract.patched_root().join("debian/control"))
                }
                None => debian_control_from_tree(
                    self.patched.get().and_then(|result| result.as_ref().ok()),
                ),
            })
            .as_ref()
    }

    fn debian_changelog(&self) -> Option<&DebianChangelog> {
        self.debian_changelog
            .get_or_init(|| match &self._extract {
                Some(extract) => {
                    DebianChangelog::load(&extract.patched_root().join("debian/changelog"))
                }
                None => debian_changelog_from_tree(
                    self.patched.get().and_then(|result| result.as_ref().ok()),
                ),
            })
            .as_ref()
    }

    fn orig_files(&self) -> Result<&FileTree, &str> {
        self.tree(&self.orig, |extract| extract.orig_files())
    }

    fn patched_files(&self) -> Result<&FileTree, &str> {
        self.tree(&self.patched, |extract| extract.patched_files())
    }

    fn tree<'a>(
        &'a self,
        cell: &'a OnceCell<Result<FileTree, String>>,
        build: impl FnOnce(&SourcePackageExtract) -> anyhow::Result<FileTree>,
    ) -> Result<&'a FileTree, &'a str> {
        match cell.get_or_init(|| match &self._extract {
            Some(extract) => build(extract).map_err(|error| error.to_string()),
            None => Err("Source package File tree is missing".to_string()),
        }) {
            Ok(tree) => Ok(tree),
            Err(message) => Err(message.as_str()),
        }
    }
}

#[derive(Clone, Copy)]
enum SourceInner<'a> {
    Tree(&'a SourceTreeData),
    Package(&'a SourcePackageData),
}

pub(crate) struct SourceContext<'a> {
    inner: SourceInner<'a>,
    sink: DiagnosticSink,
    runtime_error: Option<anyhow::Error>,
}

impl<'a> SourceContext<'a> {
    pub fn bind_tree(data: &'a SourceTreeData, rule: &(impl RuleAccess + ?Sized)) -> Self {
        Self {
            inner: SourceInner::Tree(data),
            sink: DiagnosticSink::new(rule, ProcessableType::Source),
            runtime_error: None,
        }
    }

    pub fn bind_package(data: &'a SourcePackageData, rule: &(impl RuleAccess + ?Sized)) -> Self {
        Self {
            inner: SourceInner::Package(data),
            sink: DiagnosticSink::new(rule, ProcessableType::Source),
            runtime_error: None,
        }
    }

    /// Source tree or Source package. Rules that care about Orig files or
    /// Dsc branch on this; debian/* and Patched files do not.
    #[allow(dead_code)]
    pub fn kind(&self) -> SubjectKind {
        match self.inner {
            SourceInner::Tree(_) => SubjectKind::SourceTree,
            SourceInner::Package(_) => SubjectKind::SourcePackage,
        }
    }

    /// Subject path: Source tree root or Source package `.dsc`.
    pub fn path(&self) -> &Path {
        match self.inner {
            SourceInner::Tree(data) => &data.root,
            SourceInner::Package(data) => &data.path,
        }
    }

    /// Structured `debian/control` of the packaging tree (Source tree, or
    /// Patched files). `None` if the file has a Deb822 syntax error.
    pub fn debian_control(&self) -> Option<&DebianControl> {
        match self.inner {
            SourceInner::Tree(data) => data.debian_control(),
            SourceInner::Package(data) => data.debian_control(),
        }
    }

    /// Parsed `debian/changelog` of the packaging tree. `None` if the file is
    /// missing, unreadable, or not UTF-8.
    pub fn debian_changelog(&self) -> Option<&DebianChangelog> {
        match self.inner {
            SourceInner::Tree(data) => data.debian_changelog(),
            SourceInner::Package(data) => data.debian_changelog(),
        }
    }

    /// Packaging File tree: Source tree on disk, or Source package Patched
    /// files. `None` after a global File type failure; that failure is
    /// returned from [`Self::finish`].
    pub fn files(&mut self) -> Option<&FileTree> {
        match self.inner {
            SourceInner::Tree(data) => match data.file_tree() {
                Ok(tree) => Some(tree),
                Err(message) => {
                    if self.runtime_error.is_none() {
                        self.runtime_error = Some(anyhow!("{message}"));
                    }
                    None
                }
            },
            SourceInner::Package(data) => match data.patched_files() {
                Ok(tree) => Some(tree),
                Err(message) => {
                    if self.runtime_error.is_none() {
                        self.runtime_error = Some(anyhow!("{message}"));
                    }
                    None
                }
            },
        }
    }

    /// Orig files. `None` on a Source tree, and after a global File type
    /// failure (returned from [`Self::finish`]). Native Orig files is empty.
    #[allow(dead_code)]
    pub fn orig_files(&mut self) -> Option<&FileTree> {
        match self.inner {
            SourceInner::Tree(_) => None,
            SourceInner::Package(data) => match data.orig_files() {
                Ok(tree) => Some(tree),
                Err(message) => {
                    if self.runtime_error.is_none() {
                        self.runtime_error = Some(anyhow!("{message}"));
                    }
                    None
                }
            },
        }
    }

    /// Dsc of a Source package. `None` on a Source tree, and when a tester
    /// omits it.
    pub fn dsc(&self) -> Option<&Dsc> {
        match self.inner {
            SourceInner::Tree(_) => None,
            SourceInner::Package(data) => data.dsc.as_ref(),
        }
    }

    /// Report a finding. Code and Tag come from the running Rule; Severity is
    /// set on the Diagnostic (and may be remapped later from config). Package
    /// and processable type default from this Subject when the Rule omits them.
    pub fn diagnostic(&mut self, diagnostic: Diagnostic) {
        match self.inner {
            SourceInner::Tree(data) => {
                self.sink.report(diagnostic, || data.package_name());
            }
            SourceInner::Package(data) => {
                self.sink.report(diagnostic, || data.package_name.clone());
            }
        }
    }

    pub fn finish(self) -> anyhow::Result<Vec<Diagnostic>> {
        if let Some(error) = self.runtime_error {
            return Err(error);
        }
        Ok(self.sink.into_diagnostics())
    }
}

fn debian_control_from_tree(tree: Option<&FileTree>) -> Option<DebianControl> {
    let Some(tree) = tree else {
        return Some(DebianControl::default());
    };
    let Some(entry) = tree.get("debian/control") else {
        return Some(DebianControl::default());
    };
    let Ok(bytes) = tree.read(entry) else {
        return None;
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return None;
    };
    DebianControl::parse(text)
}

fn debian_changelog_from_tree(tree: Option<&FileTree>) -> Option<DebianChangelog> {
    let tree = tree?;
    let entry = tree.get("debian/changelog")?;
    let bytes = tree.read(entry).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    Some(DebianChangelog::parse(text))
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
