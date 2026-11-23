use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use anyhow::anyhow;

use super::appstream::AppstreamFiles;
use super::binary_package_reader::BinaryPackageReader;
use super::data;
use super::deb_members::{DebMembers, Tarball};
use super::debhelper::DebhelperFacts;
use super::debian_changelog::DebianChangelog;
use super::debian_control::DebianControl;
use super::debian_copyright::DebianCopyright;
use super::diagnostic::{Diagnostic, ProcessableType};
use super::diffstat::Diffstat;
use super::diversion::Diversions;
use super::dsc::Dsc;
use super::elf::{ElfArchiveMembers, ElfFiles};
use super::file_tree::{Entry, EntryKind, FileTree};
use super::installation_control::InstallationControl;
use super::installed_changelog::{InstalledChangelog, read_installed_changelog};
use super::jar::JarFiles;
use super::lintian_overrides::LintianOverrides;
use super::openpgp::OpenPgpFile;
use super::quilt_series::QuiltSeries;
use super::rule::{Code, RuleAccess, Tag};
use super::sibling_files::SiblingFiles;
use super::source_package::{SourcePackageExtract, strip_epoch};
use super::strings::FileStrings;
use super::subject::SubjectKind;
use super::timing;
use super::type1::Type1Fonts;
use super::upstream_metadata::{self, UpstreamMetadata};

pub(super) struct DiagnosticSink {
    code: Code,
    tag: Tag,
    processable_type: ProcessableType,
    package: OnceLock<String>,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticSink {
    pub(super) fn new(
        rule: &(impl RuleAccess + ?Sized),
        processable_type: ProcessableType,
    ) -> Self {
        Self {
            code: rule.code(),
            tag: rule.tag(),
            processable_type,
            package: OnceLock::new(),
            diagnostics: Vec::new(),
        }
    }

    pub(super) fn report(
        &mut self,
        mut diagnostic: Diagnostic,
        resolve_package: impl FnOnce() -> String,
    ) {
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

    pub(super) fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

pub(crate) struct SourceTreeData {
    root: PathBuf,
    debian_control: OnceLock<Option<DebianControl>>,
    debian_changelog: OnceLock<Option<DebianChangelog>>,
    file_tree: OnceLock<Result<FileTree, String>>,
    shared: SharedSlots,
}

impl SourceTreeData {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            debian_control: OnceLock::new(),
            debian_changelog: OnceLock::new(),
            file_tree: OnceLock::new(),
            shared: SharedSlots::default(),
        }
    }

    #[cfg(test)]
    pub fn with_file_tree(root: PathBuf, file_tree: FileTree) -> Self {
        let cell = OnceLock::new();
        let _ = cell.set(Ok(file_tree));
        Self {
            root,
            debian_control: OnceLock::new(),
            debian_changelog: OnceLock::new(),
            file_tree: cell,
            shared: SharedSlots::default(),
        }
    }

    fn debian_control(&self) -> Option<&DebianControl> {
        timing::init_once(&self.debian_control, || {
            DebianControl::load(&self.root.join("debian/control"))
        })
        .as_ref()
    }

    fn debian_changelog(&self) -> Option<&DebianChangelog> {
        timing::init_once(&self.debian_changelog, || {
            DebianChangelog::load(&self.root.join("debian/changelog"))
        })
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
        match timing::init_once(&self.file_tree, || {
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
    /// Fixture ar members, for Binary packages built without a `.deb`.
    #[cfg(test)]
    fixture_deb_members: Option<DebMembers>,
    installation_control: InstallationControl,
    data_files: OnceLock<Result<FileTree, String>>,
    control_files: OnceLock<Result<FileTree, String>>,
    /// The installed Debian changelog, read on first use (see [`Self::installed_changelog`]).
    installed_changelog_cache: OnceLock<Option<InstalledChangelog>>,
    shared: SharedSlots,
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
            #[cfg(test)]
            fixture_deb_members: None,
            installation_control,
            data_files: OnceLock::new(),
            control_files: OnceLock::new(),
            installed_changelog_cache: OnceLock::new(),
            shared: SharedSlots::default(),
        })
    }

    #[cfg(test)]
    pub fn with_data_files(
        path: PathBuf,
        package_name: String,
        installation_control: InstallationControl,
        data_files: FileTree,
    ) -> Self {
        let cell = OnceLock::new();
        let _ = cell.set(Ok(data_files));
        Self {
            path,
            package_name,
            reader: None,
            #[cfg(test)]
            fixture_deb_members: None,
            installation_control,
            data_files: cell,
            control_files: OnceLock::new(),
            installed_changelog_cache: OnceLock::new(),
            shared: SharedSlots::default(),
        }
    }

    /// Fixture Control files. Without them Control files are missing.
    #[cfg(test)]
    pub fn with_control_files(self, control_files: FileTree) -> Self {
        let cell = OnceLock::new();
        let _ = cell.set(Ok(control_files));
        Self {
            control_files: cell,
            ..self
        }
    }

    /// Fixture ar members. Without them a fixture Binary package has none.
    #[cfg(test)]
    pub fn with_deb_members(self, deb_members: DebMembers) -> Self {
        Self {
            fixture_deb_members: Some(deb_members),
            ..self
        }
    }

    /// The `.deb` (`.udeb`, `.ddeb`) path.
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// `Package`, or the name guessed from the file name.
    pub(super) fn package_name(&self) -> &str {
        &self.package_name
    }

    pub(super) fn installation_control(&self) -> &InstallationControl {
        &self.installation_control
    }

    fn deb_members(&self) -> Option<&DebMembers> {
        #[cfg(test)]
        if let Some(members) = &self.fixture_deb_members {
            return Some(members);
        }
        self.reader.as_ref().map(BinaryPackageReader::deb_members)
    }

    /// The installed Debian changelog, read once from the Data files. `Err`
    /// when the Data files failed to load.
    pub(super) fn installed_changelog(&self) -> Result<Option<&InstalledChangelog>, &str> {
        let files = self.data_files()?;
        Ok(self
            .installed_changelog_cache
            .get_or_init(|| read_installed_changelog(files, &self.package_name))
            .as_ref())
    }

    pub(super) fn data_files(&self) -> Result<&FileTree, &str> {
        match timing::init_once(&self.data_files, || match &self.reader {
            Some(reader) => reader.data_file_tree().map_err(|error| error.to_string()),
            None => Err("Binary package Data files are missing".to_string()),
        }) {
            Ok(tree) => Ok(tree),
            Err(message) => Err(message.as_str()),
        }
    }

    pub(super) fn control_files(&self) -> Result<&FileTree, &str> {
        match timing::init_once(&self.control_files, || match &self.reader {
            Some(reader) => reader
                .control_file_tree()
                .map_err(|error| error.to_string()),
            None => Err("Binary package Control files are missing".to_string()),
        }) {
            Ok(tree) => Ok(tree),
            Err(message) => Err(message.as_str()),
        }
    }
}

/// What tar printed while unpacking one Binary package's Data and Control
/// tarballs (lintian's `unpack-messages`), in archive order. Read once per
/// Subject through [`BinaryPackageContext::unpack_messages`].
#[derive(Debug, Default)]
pub(crate) struct UnpackMessages {
    data: Vec<String>,
    control: Vec<String>,
}

impl UnpackMessages {
    /// The messages of one tarball. Empty when it is missing or unreadable.
    pub fn get(&self, tarball: Tarball) -> &[String] {
        match tarball {
            Tarball::Data => &self.data,
            Tarball::Control => &self.control,
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

    /// Source package name: the first word of `Source`, or this package's name.
    pub fn source_name(&self) -> String {
        self.installation_control()
            .fields()
            .get("Source")
            .and_then(|value| value.split_whitespace().next())
            .filter(|name| !name.is_empty())
            .unwrap_or(self.package_name())
            .to_string()
    }

    pub fn installation_control(&self) -> &InstallationControl {
        &self.data.installation_control
    }

    /// The Binary package is a udeb (`.udeb` file name), not a deb.
    pub fn is_udeb(&self) -> bool {
        processable_type_for_binary(&self.data.path) == ProcessableType::Udeb
    }

    /// The ar members of the `.deb`, in archive order. `None` for a fixture
    /// Binary package that has none.
    pub fn deb_members(&self) -> Option<&DebMembers> {
        self.data.deb_members()
    }

    /// The installed Debian changelog of this Binary package (lintian's
    /// `changelog`), read once for all Rules. `None` when there is none. A Data
    /// files failure is a runtime failure, as for [`Self::data_files`].
    pub fn installed_changelog(&mut self) -> Option<&'a InstalledChangelog> {
        let data: &'a BinaryPackageData = self.data;
        match data.installed_changelog() {
            Ok(changelog) => changelog,
            Err(message) => {
                if self.runtime_error.is_none() {
                    self.runtime_error = Some(anyhow!("{message}"));
                }
                None
            }
        }
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

    /// What tar printed while unpacking the Data and Control tarballs (lintian's
    /// `unpack-messages`), read once per Subject and shared by every Rule. Ask
    /// for one tarball with [`UnpackMessages::get`]. Not a File tree: a broken
    /// tarball gives no messages here, and its failure shows in the Files trees.
    pub fn unpack_messages(&mut self) -> Arc<UnpackMessages> {
        self.shared_once(|ctx| match ctx.data.reader.as_ref() {
            Some(reader) => UnpackMessages {
                data: reader.unpack_messages(Tarball::Data),
                control: reader.unpack_messages(Tarball::Control),
            },
            None => UnpackMessages::default(),
        })
    }

    /// ELF facts of the Data files ELF Entries (regular files and hardlinks),
    /// parsed once per Subject and shared by every Rule. `None` after a
    /// global File tree failure, as for [`Self::data_files`].
    pub fn data_elf_files(&mut self) -> Option<Arc<ElfFiles>> {
        self.data_files()?;
        Some(self.shared_once(|ctx| {
            ctx.data_files()
                .map(ElfFiles::from_tree)
                .unwrap_or_default()
        }))
    }

    /// ELF members of the `ar` archives among the Data files (lintian's
    /// `elf_by_member`), parsed once per Subject and shared by every Rule. Look
    /// an archive up with [`ElfArchiveMembers::get`]. `None` after a global
    /// File tree failure, as for [`Self::data_files`].
    pub fn data_elf_members(&mut self) -> Option<Arc<ElfArchiveMembers>> {
        self.data_files()?;
        Some(self.shared_once(|ctx| {
            ctx.data_files()
                .map(ElfArchiveMembers::from_tree)
                .unwrap_or_default()
        }))
    }

    /// `strings` of the ELF Data files (lintian's `$item->strings`), computed
    /// once per Subject and shared by every Rule. `None` after a global File
    /// tree failure, as for [`Self::data_files`].
    pub fn data_strings(&mut self) -> Option<Arc<FileStrings>> {
        self.data_files()?;
        Some(self.shared_once(|ctx| {
            ctx.data_files()
                .map(FileStrings::from_tree)
                .unwrap_or_default()
        }))
    }

    /// The jars among the Data files, read once per Subject and shared by
    /// every Rule. `None` after a global File tree failure, as for
    /// [`Self::data_files`].
    pub fn data_jars(&mut self) -> Option<Arc<JarFiles>> {
        self.data_files()?;
        Some(self.shared_once(|ctx| {
            ctx.data_files()
                .map(JarFiles::from_tree)
                .unwrap_or_default()
        }))
    }

    /// The Type 1 font programs among the Data files, decoded once per Subject
    /// and shared by every Rule. `None` after a global File tree failure, as
    /// for [`Self::data_files`].
    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub fn data_type1_fonts(&mut self) -> Option<Arc<Type1Fonts>> {
        self.data_files()?;
        Some(self.shared_once(|ctx| {
            ctx.data_files()
                .map(Type1Fonts::from_tree)
                .unwrap_or_default()
        }))
    }

    /// Control files File tree. `None` after a global File tree failure; that
    /// failure is returned from [`Self::finish`].
    pub fn control_files(&mut self) -> Option<&FileTree> {
        match self.data.control_files() {
            Ok(tree) => Some(tree),
            Err(message) => {
                if self.runtime_error.is_none() {
                    self.runtime_error = Some(anyhow!("{message}"));
                }
                None
            }
        }
    }

    /// Installed Lintian override file (`usr/share/lintian/overrides/<package>`,
    /// else `.gz`), parsed, not applied. `None` when there is none, and after a
    /// global File tree failure (returned from [`Self::finish`]).
    pub fn lintian_overrides(&mut self) -> Option<LintianOverrides> {
        let package = self.data.package_name.clone();
        let processable_type = processable_type_for_binary(&self.data.path);
        let files = self.data_files()?;
        LintianOverrides::installed(files, &package, processable_type)
    }

    /// Diversions the maintainer scripts declare, read once per Binary
    /// package. `None` after a global File tree failure of Control files;
    /// that failure is returned from [`Self::finish`].
    pub fn diversions(&mut self) -> Option<Arc<Diversions>> {
        self.control_files()?;
        Some(self.shared_once(|ctx| {
            ctx.control_files()
                .map(Diversions::from_control_files)
                .unwrap_or_default()
        }))
    }

    /// AppStream metadata files of the Data files (`usr/share/metainfo`,
    /// `usr/share/appdata`), shared by every Rule on the Binary package. `None`
    /// after a global File tree failure, as for [`Self::data_files`].
    #[allow(dead_code)]
    pub fn appstream_files(&mut self) -> Option<Arc<AppstreamFiles>> {
        self.data_files()?;
        Some(self.shared_once(|ctx| {
            ctx.data_files()
                .map(AppstreamFiles::from_tree)
                .unwrap_or_default()
        }))
    }

    /// Compute `build` once for this Subject. Every Rule on the Subject shares
    /// the value.
    ///
    /// The value's type is the key. `build` may use this context, and must not
    /// request `T` again (the slot initializes once, and a nested request for
    /// the same `T` waits on itself).
    pub(crate) fn shared_once<T, Build>(&mut self, build: Build) -> Arc<T>
    where
        T: Send + Sync + 'static,
        Build: FnOnce(&mut Self) -> T,
    {
        let slot = self.data.shared.slot::<T>();
        timing::init_once(&slot, || Arc::new(build(self))).clone()
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
    debian_control: OnceLock<Option<DebianControl>>,
    debian_changelog: OnceLock<Option<DebianChangelog>>,
    orig: OnceLock<Result<FileTree, String>>,
    patched: OnceLock<Result<FileTree, String>>,
    shared: SharedSlots,
    _extract: Option<SourcePackageExtract>,
}

impl SourcePackageData {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let extract = SourcePackageExtract::from_dsc(path.clone())?;
        Ok(Self {
            path,
            package_name: extract.package_name.clone(),
            dsc: Some(extract.dsc.clone()),
            debian_control: OnceLock::new(),
            debian_changelog: OnceLock::new(),
            orig: OnceLock::new(),
            patched: OnceLock::new(),
            shared: SharedSlots::default(),
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
        let orig_cell = OnceLock::new();
        let _ = orig_cell.set(Ok(orig));
        let patched_cell = OnceLock::new();
        let _ = patched_cell.set(Ok(patched));
        Self {
            path,
            package_name,
            dsc,
            debian_control: OnceLock::new(),
            debian_changelog: OnceLock::new(),
            orig: orig_cell,
            patched: patched_cell,
            shared: SharedSlots::default(),
            _extract: None,
        }
    }

    pub(super) fn debian_control(&self) -> Option<&DebianControl> {
        timing::init_once(&self.debian_control, || match &self._extract {
            Some(extract) => DebianControl::load(&extract.patched_root().join("debian/control")),
            None => {
                debian_control_from_tree(self.patched.get().and_then(|result| result.as_ref().ok()))
            }
        })
        .as_ref()
    }

    fn debian_changelog(&self) -> Option<&DebianChangelog> {
        timing::init_once(&self.debian_changelog, || match &self._extract {
            Some(extract) => {
                DebianChangelog::load(&extract.patched_root().join("debian/changelog"))
            }
            None => debian_changelog_from_tree(
                self.patched.get().and_then(|result| result.as_ref().ok()),
            ),
        })
        .as_ref()
    }

    pub(super) fn orig_files(&self) -> Result<&FileTree, &str> {
        self.tree(&self.orig, |extract| extract.orig_files())
    }

    pub(super) fn patched_files(&self) -> Result<&FileTree, &str> {
        self.tree(&self.patched, |extract| extract.patched_files())
    }

    fn tree<'a>(
        &'a self,
        cell: &'a OnceLock<Result<FileTree, String>>,
        build: impl FnOnce(&SourcePackageExtract) -> anyhow::Result<FileTree>,
    ) -> Result<&'a FileTree, &'a str> {
        match timing::init_once(cell, || match &self._extract {
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

    /// Debhelper compat level and whether `debian/rules` invokes dh.
    /// `None` after a global File type failure (returned from [`Self::finish`]).
    pub fn debhelper(&mut self) -> Option<DebhelperFacts> {
        let control = self.debian_control().cloned();
        let files = self.files()?;
        Some(DebhelperFacts::from_tree(control.as_ref(), files))
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

    /// What lintian counts as shipped in the upstream sources: on a Source
    /// tree, the whole tree; on a Native Source package, the Patched files; on
    /// any other Source package, the Orig files (without their `debian/` for
    /// `3.0 (quilt)`) plus the Patched `debian/` files. `None` after a global
    /// File type failure (returned from [`Self::finish`]).
    pub fn shipped_files(&mut self) -> Option<ShippedFiles> {
        if matches!(self.inner, SourceInner::Tree(_)) {
            let tree = self.files()?;
            return Some(ShippedFiles::new(
                file_names(tree, |_| true),
                directory_names(tree),
            ));
        }
        let quilt = self
            .dsc()
            .and_then(|dsc| dsc.fields().get("Format"))
            .is_some_and(|format| format.trim() == "3.0 (quilt)");
        let orig = self.orig_files()?;
        let native = orig.entries().is_empty();
        let orig_names = file_names(orig, |path| !(quilt && path.starts_with("debian/")));
        let orig_directories = directory_names(orig);
        let patched = self.files()?;
        if native {
            return Some(ShippedFiles::new(
                file_names(patched, |_| true),
                directory_names(patched),
            ));
        }
        let mut names = orig_names;
        names.extend(file_names(patched, |path| path.starts_with("debian/")));
        Some(ShippedFiles::new(names, orig_directories))
    }

    /// `debian/copyright` and `debian/<installable>.copyright` of the
    /// packaging tree, parsed. Symlinks, non-files and files that are not
    /// UTF-8 are left out, as lintian does. Paths are relative to the root.
    pub fn debian_copyrights(&mut self) -> Vec<(PathBuf, DebianCopyright)> {
        let mut candidates = vec![PathBuf::from("debian/copyright")];
        if let Some(control) = self.debian_control() {
            candidates.extend(control.installables().iter().map(|installable| {
                PathBuf::from(format!("debian/{}.copyright", installable.name()))
            }));
        }
        let Some(files) = self.files() else {
            return Vec::new();
        };
        candidates
            .into_iter()
            .filter_map(|path| {
                let entry = files.get(&path)?;
                if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                    return None;
                }
                let bytes = files.read(entry).ok()?;
                let text = std::str::from_utf8(&bytes).ok()?;
                Some((path, DebianCopyright::parse(text)))
            })
            .collect()
    }

    /// Source Lintian override file (`debian/source/lintian-overrides`, else
    /// `debian/source.lintian-overrides`) of the packaging tree, parsed, not
    /// applied. `None` when there is none, and after a global File tree
    /// failure (returned from [`Self::finish`]).
    #[allow(dead_code)]
    pub fn lintian_overrides(&mut self) -> Option<LintianOverrides> {
        let source_name = match self.inner {
            SourceInner::Tree(data) => data.package_name(),
            SourceInner::Package(data) => data.package_name.clone(),
        };
        let files = self.files()?;
        LintianOverrides::source(files, &source_name)
    }

    /// Quilt series files directly in `debian/patches/`, parsed: `series` and
    /// every `<vendor>.series` (lintian matches `/(.+\.)?series$` on the full
    /// path, so `.series` counts too). A symlink counts when it resolves to a
    /// file inside the tree; directories, dangling or escaping symlinks are
    /// left out. A file that is not UTF-8 is read lossily. Sorted by path,
    /// relative to the root; the path is the series' own, not its target.
    pub fn quilt_series(&mut self) -> Vec<(PathBuf, QuiltSeries)> {
        let Some(files) = self.files() else {
            return Vec::new();
        };
        files
            .query()
            .kinds([
                EntryKind::RegularFile,
                EntryKind::Hardlink,
                EntryKind::Symlink,
            ])
            .into_iter()
            .filter(|entry| is_quilt_series_path(entry.path()))
            .filter_map(|entry| {
                let bytes = files.read(follow_symlinks(files, entry)?).ok()?;
                let text = String::from_utf8_lossy(&bytes);
                Some((entry.path().to_path_buf(), QuiltSeries::parse(&text)))
            })
            .collect()
    }

    /// `debian/upstream/metadata` of the packaging tree, read and parsed once
    /// per Subject. `None` after a global File tree failure (returned from
    /// [`Self::finish`]). See [`UpstreamMetadata`] for the states.
    pub fn upstream_metadata(&mut self) -> Option<Arc<UpstreamMetadata>> {
        self.files()?;
        Some(self.shared_once(|ctx| {
            ctx.files()
                .map(upstream_metadata::read)
                .unwrap_or(UpstreamMetadata::Missing)
        }))
    }

    /// The signing key files of the packaging tree: the names of lintian's
    /// `common/signing-key-filenames` under `debian/` that resolve to a file
    /// (following symlinks), each read and parsed once per Subject, sorted by
    /// path. `None` after a global File tree failure (returned from
    /// [`Self::finish`]).
    pub fn signing_key_files(&mut self) -> Option<Arc<Vec<SigningKeyFile>>> {
        self.files()?;
        Some(self.shared_once(|ctx| ctx.read_signing_key_files()))
    }

    fn read_signing_key_files(&mut self) -> Vec<SigningKeyFile> {
        let Some(files) = self.files() else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for name in data::lines(SIGNING_KEY_FILENAMES) {
            let Some(entry) = files.get(format!("debian/{name}")) else {
                continue;
            };
            let Some(target) = follow_symlinks(files, entry) else {
                continue;
            };
            let Ok(bytes) = files.read(target) else {
                continue;
            };
            found.push(SigningKeyFile {
                path: target.path().to_path_buf(),
                key: OpenPgpFile::parse(&bytes),
            });
        }
        found.sort_by(|left, right| left.path.cmp(&right.path));
        found
    }

    /// Whether [`Self::shipped_files`] would list at least one file.
    ///
    /// Stops at the first shipped file, so a Rule that only needs to know the
    /// tree is non-empty does not allocate a path for every entry.
    pub fn has_shipped_file(&mut self) -> Option<bool> {
        if matches!(self.inner, SourceInner::Tree(_)) {
            let tree = self.files()?;
            return Some(any_file(tree, |_| true));
        }
        let quilt = self
            .dsc()
            .and_then(|dsc| dsc.fields().get("Format"))
            .is_some_and(|format| format.trim() == "3.0 (quilt)");
        let native = self.orig_files()?.entries().is_empty();
        if native {
            let patched = self.files()?;
            return Some(any_file(patched, |_| true));
        }
        let in_orig = {
            let orig = self.orig_files()?;
            any_file(orig, |path| !(quilt && path.starts_with("debian/")))
        };
        if in_orig {
            return Some(true);
        }
        let patched = self.files()?;
        Some(any_file(patched, |path| path.starts_with("debian/")))
    }

    /// Compute `build` once for this Subject. Every Rule on the Subject shares
    /// the value.
    ///
    /// The value's type is the key. `build` may use this context, and must not
    /// request `T` again (the slot initializes once, and a nested request for
    /// the same `T` waits on itself).
    pub(crate) fn shared_once<T, Build>(&mut self, build: Build) -> Arc<T>
    where
        T: Send + Sync + 'static,
        Build: FnOnce(&mut Self) -> T,
    {
        let slot = self.shared_slot::<T>();
        timing::init_once(&slot, || Arc::new(build(self))).clone()
    }

    fn shared_slot<T: Send + Sync + 'static>(&self) -> Arc<OnceLock<Arc<T>>> {
        match self.inner {
            SourceInner::Tree(data) => data.shared.slot(),
            SourceInner::Package(data) => data.shared.slot(),
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

    /// Sibling files of the `.dsc`: the files its `Files` and checksum fields
    /// list (lintian's `processable->files`), by basename, with bytes read from
    /// the directory beside the `.dsc` on request. `None` on a Source tree,
    /// which has no `.dsc`.
    pub fn sibling_files(&mut self) -> Option<Arc<SiblingFiles>> {
        match self.inner {
            SourceInner::Tree(_) => None,
            SourceInner::Package(data) => {
                Some(self.shared_once(|_| SiblingFiles::from_dsc(&data.path, data.dsc.as_ref())))
            }
        }
    }

    /// The files the Debian diff `<name>_<version>.diff.gz` changes, as
    /// lintian's `processable->diffstat` lists them. Read from the sibling
    /// files when the `.dsc` lists the diff, parsed once per Subject. Empty on
    /// a Source tree, and when there is no diff or it cannot be read.
    pub fn diffstat(&mut self) -> Arc<Diffstat> {
        self.shared_once(|ctx| ctx.read_diffstat())
    }

    fn read_diffstat(&mut self) -> Diffstat {
        let SourceInner::Package(data) = self.inner else {
            return Diffstat::default();
        };
        let Some(version) = data
            .dsc
            .as_ref()
            .and_then(|dsc| dsc.fields().get("Version"))
        else {
            return Diffstat::default();
        };
        let name = format!(
            "{}_{}.diff.gz",
            data.package_name,
            strip_epoch(version.trim())
        );
        let Some(siblings) = self.sibling_files() else {
            return Diffstat::default();
        };
        siblings
            .read(&name)
            .map_or_else(Diffstat::default, |bytes| Diffstat::from_gzip(&bytes))
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

/// Shipped files of a Source tree or Source package; see
/// [`SourceContext::shipped_files`].
pub(crate) struct ShippedFiles {
    names: Vec<String>,
    directories: HashSet<String>,
}

impl ShippedFiles {
    fn new(mut names: Vec<String>, directories: HashSet<String>) -> Self {
        names.sort();
        Self { names, directories }
    }

    /// Paths of regular files and hardlinks, sorted.
    pub fn file_names(&self) -> &[String] {
        &self.names
    }

    /// Whether the Orig files (the tree, on a Source tree) have a directory
    /// at `path`, given without a trailing slash.
    pub fn has_directory(&self, path: &str) -> bool {
        self.directories.contains(path)
    }
}

/// Facts a Rule computes once per Subject. The value's type is the key, so
/// the context does not need to name each fact.
#[derive(Default)]
struct SharedSlots {
    slots: Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl SharedSlots {
    fn slot<T: Send + Sync + 'static>(&self) -> Arc<OnceLock<Arc<T>>> {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = slots
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Arc::new(OnceLock::<Arc<T>>::new()) as Arc<dyn Any + Send + Sync>);
        Arc::downcast::<OnceLock<Arc<T>>>(Arc::clone(entry)).expect("shared slot type")
    }
}

fn any_file(tree: &FileTree, keep: impl Fn(&str) -> bool) -> bool {
    tree.entries().iter().any(|entry| {
        if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            return false;
        }
        match entry.path().to_str() {
            Some(path) => keep(path),
            None => keep(&entry.path().to_string_lossy()),
        }
    })
}

fn is_quilt_series_path(path: &Path) -> bool {
    path.parent() == Some(Path::new("debian/patches"))
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "series" || name.ends_with(".series"))
}

/// The regular file or hardlink `entry` names, following relative symlinks
/// that stay inside the tree (lintian's `is_open_ok`). `None` for directories,
/// dangling, absolute, or escaping links, and loops.
/// A signing key file of the packaging tree: the path it is found at (the
/// target's, when it is a symlink) and its parsed contents.
pub(crate) struct SigningKeyFile {
    pub path: PathBuf,
    pub key: OpenPgpFile,
}

/// Lintian's `common/signing-key-filenames`, relative to `debian/`.
const SIGNING_KEY_FILENAMES: &str = include_str!("data/common/signing-key-filenames");

pub(crate) fn follow_symlinks<'t>(tree: &'t FileTree, entry: &'t Entry) -> Option<&'t Entry> {
    let target = resolve_symlinks(tree, entry)?;
    matches!(target.kind(), EntryKind::RegularFile | EntryKind::Hardlink).then_some(target)
}

/// The entry `entry` names once its symlinks are followed, whatever its kind
/// (lintian's `resolve_path`). `None` when a link dangles, is absolute or
/// escapes the tree, or loops.
pub(crate) fn resolve_symlinks<'t>(tree: &'t FileTree, entry: &'t Entry) -> Option<&'t Entry> {
    let mut current = entry;
    for _ in 0..8 {
        if current.kind() != EntryKind::Symlink {
            return Some(current);
        }
        let mut resolved = current.path().parent()?.to_path_buf();
        for component in current.link_target()?.components() {
            match component {
                Component::Normal(part) => resolved.push(part),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !resolved.pop() {
                        return None;
                    }
                }
                Component::RootDir | Component::Prefix(_) => return None,
            }
        }
        current = tree.get(&resolved)?;
    }
    None
}

fn file_names(tree: &FileTree, keep: impl Fn(&str) -> bool) -> Vec<String> {
    tree.query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .filter(|path| keep(path))
        .collect()
}

fn directory_names(tree: &FileTree) -> HashSet<String> {
    tree.query()
        .kinds([EntryKind::Directory])
        .into_iter()
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .collect()
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

pub(super) fn processable_type_for_binary(path: &Path) -> ProcessableType {
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
