//! Contexts of the Rules that inspect the artifacts of a Group: the Changes
//! file, the Buildinfo file, and the Group as a whole (ADR 0055).

use std::path::{Path, PathBuf};

use super::buildinfo::Buildinfo;
use super::changes::Changes;
use super::context::{
    BinaryPackageData, DiagnosticSink, SourcePackageData, processable_type_for_binary,
};
use super::debian_control::{ControlParagraph, DebianControl};
use super::diagnostic::{Diagnostic, ProcessableType};
use super::dsc::Dsc;
use super::file_tree::FileTree;
use super::group::{BuildinfoData, ChangesData, Group};
use super::package_relation::PackageRelation;
use super::rule::RuleAccess;

/// What a Rule of the Changes file kind sees. Diagnostics get the `Source` of
/// the file (else a name guessed from its file name) and processable type
/// `changes`.
pub(crate) struct ChangesContext<'a> {
    data: &'a ChangesData,
    sink: DiagnosticSink,
}

#[allow(dead_code)]
impl<'a> ChangesContext<'a> {
    pub fn bind(data: &'a ChangesData, rule: &(impl RuleAccess + ?Sized)) -> Self {
        Self {
            data,
            sink: DiagnosticSink::new(rule, ProcessableType::Changes),
        }
    }

    /// The `.changes` path.
    pub fn path(&self) -> &Path {
        &self.data.path
    }

    pub fn package_name(&self) -> &str {
        &self.data.package_name
    }

    pub fn changes(&self) -> &Changes {
        &self.data.changes
    }

    /// The path of a file in the directory of the `.changes`, where the files
    /// it names are. `name` is a base name from the `Files` field.
    pub fn sibling(&self, name: &str) -> PathBuf {
        self.data
            .path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(name)
    }

    /// Report a finding. Code and Tag come from the running Rule; package and
    /// processable type default from the Changes file when the Rule omits them.
    pub fn diagnostic(&mut self, diagnostic: Diagnostic) {
        let data = self.data;
        self.sink.report(diagnostic, || data.package_name.clone());
    }

    pub fn finish(self) -> Vec<Diagnostic> {
        self.sink.into_diagnostics()
    }
}

/// What a Rule of the Buildinfo file kind sees. Diagnostics get the `Source`
/// of the file (else a name guessed from its file name) and processable type
/// `buildinfo`.
pub(crate) struct BuildinfoContext<'a> {
    data: &'a BuildinfoData,
    sink: DiagnosticSink,
}

#[allow(dead_code)]
impl<'a> BuildinfoContext<'a> {
    pub fn bind(data: &'a BuildinfoData, rule: &(impl RuleAccess + ?Sized)) -> Self {
        Self {
            data,
            sink: DiagnosticSink::new(rule, ProcessableType::Buildinfo),
        }
    }

    /// The `.buildinfo` path.
    pub fn path(&self) -> &Path {
        &self.data.path
    }

    pub fn package_name(&self) -> &str {
        &self.data.package_name
    }

    pub fn buildinfo(&self) -> &Buildinfo {
        &self.data.buildinfo
    }

    /// The path of a file in the directory of the `.buildinfo`.
    pub fn sibling(&self, name: &str) -> PathBuf {
        self.data
            .path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(name)
    }

    /// Report a finding; see [`ChangesContext::diagnostic`].
    pub fn diagnostic(&mut self, diagnostic: Diagnostic) {
        let data = self.data;
        self.sink.report(diagnostic, || data.package_name.clone());
    }

    pub fn finish(self) -> Vec<Diagnostic> {
        self.sink.into_diagnostics()
    }
}

/// One installable of a Group (a `.deb`, `.udeb` or `.ddeb`).
#[derive(Clone, Copy)]
pub(crate) struct GroupInstallable<'a> {
    group: &'a Group,
    index: usize,
}

#[allow(dead_code)]
impl<'a> GroupInstallable<'a> {
    fn data(&self) -> &'a BinaryPackageData {
        &self.group.binaries[self.index]
    }

    /// `Package`, or the name guessed from the file name.
    pub fn name(&self) -> &'a str {
        self.data().package_name()
    }

    pub fn path(&self) -> &'a Path {
        self.data().path()
    }

    pub fn version(&self) -> &'a str {
        self.field("Version")
    }

    pub fn architecture(&self) -> &'a str {
        self.field("Architecture")
    }

    /// `Multi-Arch`, empty when the installable has none.
    pub fn multi_arch(&self) -> &'a str {
        self.field("Multi-Arch")
    }

    fn field(&self, name: &str) -> &'a str {
        self.fields().get(name).unwrap_or_default().trim()
    }

    /// The installable is a udeb (`.udeb` file name), not a deb or ddeb.
    pub fn is_udeb(&self) -> bool {
        self.processable_type() == ProcessableType::Udeb
    }

    /// The processable type lintian files the installable under: `udeb` for a
    /// `.udeb`, else `binary`.
    pub fn processable_type(&self) -> ProcessableType {
        processable_type_for_binary(self.path())
    }

    /// The fields of its Installation control.
    pub fn fields(&self) -> &'a ControlParagraph {
        self.data().installation_control().fields()
    }

    /// The names in `Provides`: comma separated, each trimmed, empty entries
    /// left out (lintian's `trimmed_list('Provides', qr{,})`).
    pub fn provides(&self) -> Vec<&'a str> {
        self.fields()
            .get("Provides")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .collect()
    }

    /// A relation field of its Installation control (`Depends`, `Conflicts`,
    /// `Replaces`, ...); empty when it has none.
    pub(crate) fn relation(&self, field: &str) -> PackageRelation {
        self.fields().relation(field).unwrap_or_default()
    }

    /// Pre-Depends and Depends together: lintian's `strong` relation.
    pub(crate) fn strong_relation(&self) -> PackageRelation {
        self.fields().relations_and(&["Pre-Depends", "Depends"])
    }
}

/// What a Rule of the Group kind sees: the Changes file, the Dsc, the
/// Buildinfo file and the installables of one source package and version,
/// each when the Group has it; the files of every installable; and the
/// source's own Debian control and files. Diagnostics attach to the Source
/// package: package is the Group's source name and processable type is
/// `source`.
pub(crate) struct GroupContext<'a> {
    group: &'a Group,
    sink: DiagnosticSink,
    runtime_error: Option<anyhow::Error>,
}

#[allow(dead_code)]
impl<'a> GroupContext<'a> {
    pub fn bind(group: &'a Group, rule: &(impl RuleAccess + ?Sized)) -> Self {
        Self {
            group,
            sink: DiagnosticSink::new(rule, ProcessableType::Source),
            runtime_error: None,
        }
    }

    /// The source package name of the Group.
    pub fn source_name(&self) -> &str {
        self.group.source_name()
    }

    pub fn changes(&self) -> Option<&'a Changes> {
        self.group.changes.as_ref().map(|data| &data.changes)
    }

    /// The Dsc of the Group's `.dsc`.
    pub fn dsc(&self) -> Option<&'a Dsc> {
        self.group.source.as_ref().map(|data| &data.dsc)
    }

    pub fn buildinfo(&self) -> Option<&'a Buildinfo> {
        self.group.buildinfo.as_ref().map(|data| &data.buildinfo)
    }

    /// Every installable, debs and ddebs before udebs, each in path order.
    pub fn installables(&self) -> Vec<GroupInstallable<'a>> {
        (0..self.group.binaries.len())
            .map(|index| GroupInstallable {
                group: self.group,
                index,
            })
            .collect()
    }

    /// The installables of the Group that `installable` strongly depends on:
    /// what its Pre-Depends and Depends name (lintian's
    /// `Group::direct_dependencies`). Weak relations and self-dependencies do
    /// not count.
    pub fn direct_dependencies(
        &self,
        installable: &GroupInstallable<'a>,
    ) -> Vec<GroupInstallable<'a>> {
        self.group
            .direct_dependencies()
            .get(installable.name())
            .map(|indexes| {
                indexes
                    .iter()
                    .map(|&index| GroupInstallable {
                        group: self.group,
                        index,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The installables of the Group whose Pre-Depends and Depends name
    /// `installable` (lintian's `Group::direct_reliants`): the inverse of
    /// [`Self::direct_dependencies`], with the same exclusions.
    pub fn direct_reliants(&self, installable: &GroupInstallable<'a>) -> Vec<GroupInstallable<'a>> {
        self.group
            .direct_reliants()
            .get(installable.name())
            .map(|indexes| {
                indexes
                    .iter()
                    .map(|&index| GroupInstallable {
                        group: self.group,
                        index,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Data files of an installable: the files as they would land on disk, a
    /// File tree like the one a Binary-package Rule walks, read once and
    /// shared with it. `None` after a global File tree failure; that failure
    /// is returned from [`Self::finish`].
    pub fn data_files(&mut self, installable: &GroupInstallable<'a>) -> Option<&'a FileTree> {
        match installable.data().data_files() {
            Ok(tree) => Some(tree),
            Err(message) => {
                self.fail(message);
                None
            }
        }
    }

    /// Control files of an installable; see [`Self::data_files`].
    pub fn control_files(&mut self, installable: &GroupInstallable<'a>) -> Option<&'a FileTree> {
        match installable.data().control_files() {
            Ok(tree) => Some(tree),
            Err(message) => {
                self.fail(message);
                None
            }
        }
    }

    /// The Source package of the Group's `.dsc`, extracted once and shared
    /// with the Source-package Rules. `None` when the Group has no `.dsc`,
    /// and after an extract failure (returned from [`Self::finish`]).
    fn source_package(&mut self) -> Option<&'a SourcePackageData> {
        let source = self.group.source.as_ref()?;
        match source.package() {
            Ok(package) => Some(package),
            Err(message) => {
                self.fail(message);
                None
            }
        }
    }

    /// Debian control of the source (the Patched files' `debian/control`).
    /// `None` without a `.dsc`, after an extract failure, and when the file
    /// has a Deb822 syntax error.
    pub fn debian_control(&mut self) -> Option<&'a DebianControl> {
        self.source_package()?.debian_control()
    }

    /// Patched files of the source; see [`Self::debian_control`].
    pub fn patched_files(&mut self) -> Option<&'a FileTree> {
        match self.source_package()?.patched_files() {
            Ok(tree) => Some(tree),
            Err(message) => {
                self.fail(message);
                None
            }
        }
    }

    /// Orig files of the source (empty when Native); see
    /// [`Self::debian_control`].
    pub fn orig_files(&mut self) -> Option<&'a FileTree> {
        match self.source_package()?.orig_files() {
            Ok(tree) => Some(tree),
            Err(message) => {
                self.fail(message);
                None
            }
        }
    }

    fn fail(&mut self, message: &str) {
        if self.runtime_error.is_none() {
            self.runtime_error = Some(anyhow::anyhow!("{message}"));
        }
    }

    /// Report a finding; package defaults to the Group's source name.
    pub fn diagnostic(&mut self, diagnostic: Diagnostic) {
        let group = self.group;
        self.sink
            .report(diagnostic, || group.source_name().to_string());
    }

    /// Report a finding on an installable of the Group instead of on the
    /// Source package: the Diagnostic gets the installable's name and its
    /// processable type (`binary` or `udeb`). For the Tags lintian emits on a
    /// Binary package from a Check that needs its siblings.
    pub fn installable_diagnostic(
        &mut self,
        installable: &GroupInstallable<'a>,
        diagnostic: Diagnostic,
    ) {
        self.diagnostic(
            diagnostic
                .with_package(installable.name())
                .with_processable_type(installable.processable_type()),
        );
    }

    /// Report on the Group's `.changes`: the Diagnostic gets the processable
    /// type `changes` (the source name stays its package). For the Tags
    /// lintian emits on a `.changes` from a Check that needs the `.dsc`.
    pub fn changes_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostic(diagnostic.with_processable_type(ProcessableType::Changes));
    }

    pub fn finish(self) -> anyhow::Result<Vec<Diagnostic>> {
        if let Some(error) = self.runtime_error {
            return Err(error);
        }
        Ok(self.sink.into_diagnostics())
    }
}
