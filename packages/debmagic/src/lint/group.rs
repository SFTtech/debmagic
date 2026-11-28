//! Group assembly: the artifacts of one source package and version.
//!
//! Lintian's `bin/lintian` and `Lintian::Pool`: every path on the command line
//! becomes a Group; a `.changes`, `.buildinfo` or `.dsc` pulls in the sibling
//! files it names from the same directory (`.deb`, `.udeb`, `.ddeb`, `.dsc`,
//! `.buildinfo`; a name with a `/` is ignored, a named file that is missing is
//! an error); Groups with the same `<source>_<version>` merge. A Group has at
//! most one `.changes`, one `.dsc` and one `.buildinfo` (the first wins when
//! Groups merge, and a later `.buildinfo` is ignored) and any number of
//! installables, deduplicated by type, name, version and architecture.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, OnceLock};

use anyhow::bail;
use regex::Regex;

use super::binary_package_reader::guess_package_name;
use super::buildinfo::Buildinfo;
use super::changes::Changes;
use super::context::{BinaryPackageData, SourcePackageData, processable_type_for_binary};
use super::diagnostic::ProcessableType;
use super::dsc::Dsc;
use super::file_list::FileList;
use super::subject::Subject;
use super::timing;

/// `Source: name (version)` of a binary package whose version differs from its
/// source's.
static SOURCE_WITH_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\S+)\s*\(([^)]+)\)").expect("valid regex"));

/// The `.changes` of a Group.
pub(crate) struct ChangesData {
    #[allow(dead_code)]
    pub path: PathBuf,
    /// `Source`, or the name guessed from the file name.
    pub package_name: String,
    pub changes: Changes,
}

/// The `.buildinfo` of a Group.
pub(crate) struct BuildinfoData {
    pub path: PathBuf,
    /// `Source`, or the name guessed from the file name.
    pub package_name: String,
    pub buildinfo: Buildinfo,
}

/// The `.dsc` of a Group: the parsed paragraph, and the Source package once a
/// Rule asks for it. Extracting the Source package (`dpkg-source`) is shared
/// by the Rules of the Source package kind and the Group Rules that read the
/// source's Debian control or files.
pub(crate) struct DscData {
    pub path: PathBuf,
    pub dsc: Dsc,
    package: OnceLock<Result<SourcePackageData, String>>,
}

impl DscData {
    pub fn new(path: PathBuf, dsc: Dsc) -> Self {
        Self {
            path,
            dsc,
            package: OnceLock::new(),
        }
    }

    /// A `.dsc` whose Source package is already there, for fixtures.
    #[cfg(test)]
    pub fn with_package(path: PathBuf, dsc: Dsc, package: SourcePackageData) -> Self {
        let cell = OnceLock::new();
        let _ = cell.set(Ok(package));
        Self {
            path,
            dsc,
            package: cell,
        }
    }

    /// The extracted Source package: Orig files, Patched files and Debian
    /// control. An extract failure is the same for every caller.
    pub fn package(&self) -> Result<&SourcePackageData, &str> {
        match timing::init_once(&self.package, || {
            SourcePackageData::new(self.path.clone()).map_err(|error| format!("{error:#}"))
        }) {
            Ok(package) => Ok(package),
            Err(message) => Err(message.as_str()),
        }
    }
}

pub(crate) struct Group {
    source_name: String,
    source_version: String,
    pub changes: Option<ChangesData>,
    pub source: Option<DscData>,
    pub buildinfo: Option<BuildinfoData>,
    /// Installables: debs and ddebs first, then udebs, each by path.
    pub binaries: Vec<BinaryPackageData>,
    direct_dependencies: OnceLock<HashMap<String, Vec<usize>>>,
    direct_reliants: OnceLock<HashMap<String, Vec<usize>>>,
}

enum Member {
    Changes(ChangesData),
    Source(Box<DscData>),
    Buildinfo(BuildinfoData),
    Binary(Box<BinaryPackageData>),
}

/// What identifies a member in the pool, and what it names.
struct Identity {
    source_name: String,
    source_version: String,
    files: Option<FileList>,
}

impl Group {
    fn empty() -> Self {
        Self {
            source_name: String::new(),
            source_version: String::new(),
            changes: None,
            source: None,
            buildinfo: None,
            binaries: Vec::new(),
            direct_dependencies: OnceLock::new(),
            direct_reliants: OnceLock::new(),
        }
    }

    /// A Group built from parts, for fixtures.
    #[cfg(test)]
    pub fn fixture(
        source_name: &str,
        source_version: &str,
        changes: Option<ChangesData>,
        source: Option<DscData>,
        buildinfo: Option<BuildinfoData>,
        binaries: Vec<BinaryPackageData>,
    ) -> Self {
        let mut group = Self::empty();
        group.source_name = source_name.to_string();
        group.source_version = source_version.to_string();
        group.changes = changes;
        group.source = source;
        group.buildinfo = buildinfo;
        group.binaries = binaries;
        group
    }

    /// Lintian's `Group::name`: `<source>_<version>`, empty when either part
    /// is missing.
    pub fn name(&self) -> String {
        if self.source_name.is_empty() || self.source_version.is_empty() {
            String::new()
        } else {
            format!("{}_{}", self.source_name, self.source_version)
        }
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    /// Open the artifacts and group them as lintian does. Groups come in the
    /// order their first artifact was named. A path that cannot be opened is
    /// an error for the whole run.
    pub fn assemble(subjects: &[Subject]) -> anyhow::Result<Vec<Self>> {
        let mut pool: Vec<Self> = Vec::new();
        for subject in subjects {
            let path = subject.path();
            if !path.is_file() {
                bail!("{} is not a readable file", path.display());
            }
            let mut group = Self::empty();
            let (member, identity) = open_member(path, subject)?;
            let files = identity.files.clone();
            group.add(member, identity)?;
            if let Some(files) = files {
                let parent = path.parent().unwrap_or_else(|| Path::new("."));
                for file in files.files() {
                    let sibling = parent.join(file.name());
                    if !sibling.exists() {
                        bail!("{} does not exist", sibling.display());
                    }
                    let Some(subject) = sibling_subject(&sibling) else {
                        continue;
                    };
                    let (member, identity) = open_member(&sibling, &subject)?;
                    group.add(member, identity)?;
                }
            }
            pool.merge(group);
        }
        for group in &mut pool {
            group.binaries.sort_by(|left, right| {
                let udeb = |binary: &BinaryPackageData| {
                    processable_type_for_binary(binary.path()) == ProcessableType::Udeb
                };
                (udeb(left), left.path()).cmp(&(udeb(right), right.path()))
            });
        }
        Ok(pool)
    }

    /// Lintian's `Group::add_processable`. `Ok(false)`: not added (another
    /// source package or version, a second `.buildinfo`, a binary already in).
    fn add(&mut self, member: Member, identity: Identity) -> anyhow::Result<bool> {
        if self.source_name.is_empty() {
            self.source_name = identity.source_name.clone();
        }
        if self.source_version.is_empty() {
            self.source_version = identity.source_version.clone();
        }
        if self.source_name != identity.source_name
            || self.source_version != identity.source_version
        {
            return Ok(false);
        }
        match member {
            Member::Changes(changes) => {
                if self.changes.is_some() {
                    bail!("cannot add another changes file");
                }
                self.changes = Some(changes);
            }
            Member::Source(source) => {
                if self.source.is_some() {
                    bail!("cannot add another source package");
                }
                self.source = Some(*source);
            }
            Member::Buildinfo(buildinfo) => {
                if self.buildinfo.is_some() {
                    return Ok(false);
                }
                self.buildinfo = Some(buildinfo);
            }
            Member::Binary(binary) => {
                if self
                    .binaries
                    .iter()
                    .any(|known| same_installable(known, &binary))
                {
                    return Ok(false);
                }
                self.binaries.push(*binary);
            }
        }
        Ok(true)
    }

    /// Lintian's `Group::direct_dependencies`, for every installable: the
    /// installables of this Group that its Pre-Depends and Depends name
    /// (satisfy), by position in [`Self::binaries`], keyed by package name.
    /// Self-dependencies are left out. Installables with the same name (one
    /// per architecture) share the entry of the last of them, as in lintian.
    pub fn direct_dependencies(&self) -> &HashMap<String, Vec<usize>> {
        self.direct_dependencies.get_or_init(|| {
            let mut dependencies = HashMap::new();
            for that in &self.binaries {
                let relation = that
                    .installation_control()
                    .fields()
                    .relations_and(&["Pre-Depends", "Depends"]);
                let specific = self
                    .binaries
                    .iter()
                    .enumerate()
                    .filter(|(_, this)| this.package_name() != that.package_name())
                    .filter(|(_, this)| relation.satisfies(this.package_name()))
                    .map(|(index, _)| index)
                    .collect();
                dependencies.insert(that.package_name().to_string(), specific);
            }
            dependencies
        })
    }

    /// Lintian's `Group::direct_reliants`, for every installable: the
    /// installables of this Group whose strong relations (Pre-Depends and
    /// Depends) satisfy it, by position in [`Self::binaries`], keyed by
    /// package name. The inverse of [`Self::direct_dependencies`]. Self-
    /// dependencies are left out. Installables with the same name share the
    /// entry of the last of them, as in lintian.
    pub fn direct_reliants(&self) -> &HashMap<String, Vec<usize>> {
        self.direct_reliants.get_or_init(|| {
            let strong: Vec<_> = self
                .binaries
                .iter()
                .map(|binary| {
                    binary
                        .installation_control()
                        .fields()
                        .relations_and(&["Pre-Depends", "Depends"])
                })
                .collect();
            let mut reliants = HashMap::new();
            for that in &self.binaries {
                let specific = self
                    .binaries
                    .iter()
                    .enumerate()
                    .filter(|(_, this)| this.package_name() != that.package_name())
                    .filter(|(index, _)| strong[*index].satisfies(that.package_name()))
                    .map(|(index, _)| index)
                    .collect();
                reliants.insert(that.package_name().to_string(), specific);
            }
            reliants
        })
    }
}

trait Pool {
    fn merge(&mut self, group: Group);
}

impl Pool for Vec<Group> {
    /// Lintian's `Pool::add_group`: a Group with a known name merges into the
    /// first Group of that name, keeping what the old one already has.
    fn merge(&mut self, group: Group) {
        let name = group.name();
        let Some(old) = self.iter_mut().find(|old| old.name() == name) else {
            self.push(group);
            return;
        };
        let Group {
            changes,
            source,
            buildinfo,
            binaries,
            ..
        } = group;
        old.changes = old.changes.take().or(changes);
        old.source = old.source.take().or(source);
        old.buildinfo = old.buildinfo.take().or(buildinfo);
        for binary in binaries {
            if !old
                .binaries
                .iter()
                .any(|known| same_installable(known, &binary))
            {
                old.binaries.push(binary);
            }
        }
        old.direct_dependencies = OnceLock::new();
        old.direct_reliants = OnceLock::new();
    }
}

fn same_installable(left: &BinaryPackageData, right: &BinaryPackageData) -> bool {
    installable_id(left) == installable_id(right)
}

/// Lintian's `Processable::identifier` without the type prefix a hash per
/// type already gives: type, name, version, architecture.
fn installable_id(binary: &BinaryPackageData) -> (bool, String, String, String) {
    let fields = binary.installation_control().fields();
    let field = |name: &str| fields.get(name).unwrap_or_default().trim().to_string();
    (
        processable_type_for_binary(binary.path()) == ProcessableType::Udeb,
        binary.package_name().to_string(),
        field("Version"),
        field("Architecture"),
    )
}

/// The Subject a file named in a `Files` field is, if lintian looks at it:
/// `.deb`, `.udeb`, `.ddeb`, `.dsc` and `.buildinfo`.
fn sibling_subject(path: &Path) -> Option<Subject> {
    let absolute = path.to_path_buf();
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("deb" | "udeb" | "ddeb") => Some(Subject::BinaryPackage(absolute)),
        Some("dsc") => Some(Subject::SourcePackage(absolute)),
        Some("buildinfo") => Some(Subject::Buildinfo(absolute)),
        _ => None,
    }
}

fn open_member(path: &Path, subject: &Subject) -> anyhow::Result<(Member, Identity)> {
    match subject {
        Subject::SourceTree(_) => bail!(
            "{} is a Source tree, which is not part of a Group",
            path.display()
        ),
        Subject::Changes(_) => {
            let changes = Changes::load(path)?;
            let (package_name, identity) = named_identity(
                path,
                changes.source(),
                changes.version(),
                changes.files().clone(),
            )?;
            Ok((
                Member::Changes(ChangesData {
                    path: path.to_path_buf(),
                    package_name,
                    changes,
                }),
                identity,
            ))
        }
        Subject::Buildinfo(_) => {
            let buildinfo = Buildinfo::load(path)?;
            let (package_name, identity) = named_identity(
                path,
                buildinfo.source(),
                buildinfo.version(),
                buildinfo.files().clone(),
            )?;
            Ok((
                Member::Buildinfo(BuildinfoData {
                    path: path.to_path_buf(),
                    package_name,
                    buildinfo,
                }),
                identity,
            ))
        }
        Subject::SourcePackage(_) => {
            let dsc = Dsc::load(path)?;
            let name = dsc
                .fields()
                .get("Source")
                .map(str::trim)
                .unwrap_or_default()
                .to_string();
            if name.is_empty() {
                bail!("{} is missing Source field", path.display());
            }
            let identity = Identity {
                source_name: name,
                source_version: dsc
                    .fields()
                    .get("Version")
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_string(),
                files: Some(FileList::parse(dsc.fields(), false)),
            };
            Ok((
                Member::Source(Box::new(DscData::new(path.to_path_buf(), dsc))),
                identity,
            ))
        }
        Subject::BinaryPackage(_) => {
            let binary = BinaryPackageData::new(path.to_path_buf())?;
            let fields = binary.installation_control().fields();
            let version = fields.get("Version").unwrap_or_default().trim().to_string();
            let source = fields.get("Source").unwrap_or_default().trim();
            let (source_name, source_version) = match SOURCE_WITH_VERSION.captures(source) {
                Some(captures) => (captures[1].to_string(), captures[2].trim().to_string()),
                None if source.is_empty() => (binary.package_name().to_string(), version),
                None => (source.to_string(), version),
            };
            Ok((
                Member::Binary(Box::new(binary)),
                Identity {
                    source_name,
                    source_version,
                    files: None,
                },
            ))
        }
    }
}

/// The name and identity of a changes or buildinfo file: `Source` (else a
/// guess from the file name) and `Version`.
fn named_identity(
    path: &Path,
    source: Option<&str>,
    version: Option<&str>,
    files: FileList,
) -> anyhow::Result<(String, Identity)> {
    let name = match source.filter(|name| !name.is_empty()) {
        Some(name) => name.to_string(),
        None => {
            let guess = guess_package_name(path);
            if guess.is_empty() {
                bail!("cannot determine the name from {}", path.display());
            }
            guess
        }
    };
    Ok((
        name.clone(),
        Identity {
            source_name: name,
            source_version: version.unwrap_or_default().to_string(),
            files: Some(files),
        },
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::lint::binary_package_reader::write_test_deb;

    fn deb(dir: &Path, file: &str, control: &str) -> PathBuf {
        let path = dir.join(file);
        write_test_deb(&path, control).expect("writing a test deb");
        path
    }

    fn control(package: &str, source: &str, version: &str, arch: &str) -> String {
        format!(
            "Package: {package}\nSource: {source}\nVersion: {version}\nArchitecture: {arch}\n\
             Maintainer: A <a@example.org>\nDescription: x\n y\n"
        )
    }

    fn changes(dir: &Path, name: &str, source: &str, version: &str, files: &[&str]) -> PathBuf {
        let mut text = format!("Source: {source}\nVersion: {version}\nFiles:\n");
        for file in files {
            text.push_str(&format!(
                " d41d8cd98f00b204e9800998ecf8427e 0 misc optional {file}\n"
            ));
        }
        let path = dir.join(name);
        fs::write(&path, text).expect("writing a changes file");
        path
    }

    fn subject(path: &Path) -> Subject {
        crate::lint::subject::resolve_subject(path.to_path_buf()).expect("subject")
    }

    #[test]
    fn changes_pulls_in_the_debs_it_names_and_ignores_the_rest() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        deb(
            dir.path(),
            "a_1.0_all.deb",
            &control("a", "src", "1.0", "all"),
        );
        deb(
            dir.path(),
            "a-dev_1.0_all.deb",
            &control("a-dev", "src (1.0)", "1.0+b1", "all"),
        );
        fs::write(dir.path().join("src_1.0.tar.xz"), b"")?;
        let path = changes(
            dir.path(),
            "src_1.0_all.changes",
            "src",
            "1.0",
            &[
                "a_1.0_all.deb",
                "a-dev_1.0_all.deb",
                "src_1.0.tar.xz",
                "../evil.deb",
            ],
        );
        let groups = Group::assemble(&[subject(&path)])?;
        assert_eq!(groups.len(), 1);
        let group = &groups[0];
        assert_eq!(group.name(), "src_1.0");
        assert!(group.changes.is_some());
        assert!(group.source.is_none());
        let names: Vec<_> = group
            .binaries
            .iter()
            .map(BinaryPackageData::package_name)
            .collect();
        // Installables sort by path, and `-` sorts before `_`.
        assert_eq!(names, ["a-dev", "a"]);
        Ok(())
    }

    #[test]
    fn a_named_file_that_is_missing_is_an_error() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = changes(
            dir.path(),
            "src_1.0_all.changes",
            "src",
            "1.0",
            &["gone_1.0_all.deb"],
        );
        let error = Group::assemble(&[subject(&path)]).err().expect("error");
        assert!(
            error
                .to_string()
                .contains("gone_1.0_all.deb does not exist"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn arguments_with_the_same_source_and_version_merge() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let first = deb(
            dir.path(),
            "a_1.0_all.deb",
            &control("a", "src", "1.0", "all"),
        );
        let second = deb(
            dir.path(),
            "b_1.0_all.deb",
            &control("b", "src", "1.0", "all"),
        );
        let other = deb(
            dir.path(),
            "c_2.0_all.deb",
            &control("c", "other", "2.0", "all"),
        );
        let changes = changes(
            dir.path(),
            "src_1.0_all.changes",
            "src",
            "1.0",
            &["a_1.0_all.deb"],
        );
        let groups = Group::assemble(&[
            subject(&first),
            subject(&other),
            subject(&second),
            subject(&changes),
            subject(&first),
        ])?;
        let names: Vec<_> = groups.iter().map(Group::name).collect();
        assert_eq!(names, ["src_1.0", "other_2.0"]);
        assert!(groups[0].changes.is_some());
        let binaries: Vec<_> = groups[0]
            .binaries
            .iter()
            .map(BinaryPackageData::package_name)
            .collect();
        assert_eq!(binaries, ["a", "b"]);
        Ok(())
    }

    #[test]
    fn a_sibling_of_another_source_is_not_added() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        deb(
            dir.path(),
            "x_1.0_all.deb",
            &control("x", "elsewhere", "1.0", "all"),
        );
        let path = changes(
            dir.path(),
            "src_1.0_all.changes",
            "src",
            "1.0",
            &["x_1.0_all.deb"],
        );
        let groups = Group::assemble(&[subject(&path)])?;
        assert!(groups[0].binaries.is_empty());
        Ok(())
    }

    #[test]
    fn a_binary_without_a_source_field_is_its_own_source() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let udeb = deb(
            dir.path(),
            "a-udeb_1.0_all.udeb",
            "Package: a-udeb\nVersion: 1.0\nArchitecture: all\n",
        );
        let plain = deb(
            dir.path(),
            "z_1.0_all.deb",
            "Package: z\nVersion: 1.0\nArchitecture: all\n",
        );
        let groups = Group::assemble(&[subject(&udeb), subject(&plain)])?;
        // No `Source` field: a package is its own source, so two groups.
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name(), "a-udeb_1.0");
        Ok(())
    }

    #[test]
    fn direct_dependencies_are_strong_and_exclude_self() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let mut paths = Vec::new();
        for (name, extra) in [
            ("a", "Depends: b (>= 1), a\n"),
            ("b", "Pre-Depends: c\nRecommends: a\n"),
            ("c", ""),
        ] {
            let text = format!("{}{extra}", control(name, "src", "1.0", "all"));
            paths.push(subject(&deb(
                dir.path(),
                &format!("{name}_1.0_all.deb"),
                &text,
            )));
        }
        let groups = Group::assemble(&paths)?;
        let dependencies = groups[0].direct_dependencies();
        assert_eq!(dependencies["a"], [1]);
        assert_eq!(dependencies["b"], [2]);
        assert!(dependencies["c"].is_empty());
        Ok(())
    }

    #[test]
    fn direct_reliants_are_strong_and_exclude_self() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let mut paths = Vec::new();
        for (name, extra) in [
            ("a", "Depends: b (>= 1), a\n"),
            ("b", "Pre-Depends: c\nRecommends: a\n"),
            ("c", ""),
        ] {
            let text = format!("{}{extra}", control(name, "src", "1.0", "all"));
            paths.push(subject(&deb(
                dir.path(),
                &format!("{name}_1.0_all.deb"),
                &text,
            )));
        }
        let groups = Group::assemble(&paths)?;
        let reliants = groups[0].direct_reliants();
        // Only `a` depends strongly on `b`; `b`'s Recommends does not count,
        // and `a`'s self-dependency is left out.
        assert_eq!(reliants["b"], [0]);
        assert_eq!(reliants["c"], [1]);
        assert!(reliants["a"].is_empty());
        Ok(())
    }

    #[test]
    fn a_dsc_pulls_in_the_files_it_names() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let dsc = dir.path().join("src_1.0.dsc");
        fs::write(
            &dsc,
            "Source: src\nVersion: 1.0\nFiles:\n abc 0 src_1.0.tar.xz\n",
        )?;
        let error = Group::assemble(&[subject(&dsc)]).err().expect("error");
        assert!(
            error.to_string().contains("src_1.0.tar.xz does not exist"),
            "{error}"
        );
        fs::write(dir.path().join("src_1.0.tar.xz"), b"")?;
        let groups = Group::assemble(&[subject(&dsc)])?;
        assert!(groups[0].source.is_some());
        Ok(())
    }

    #[test]
    fn a_second_buildinfo_is_ignored() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let first = dir.path().join("src_1.0_amd64.buildinfo");
        let second = dir.path().join("src_1.0_all.buildinfo");
        fs::write(&first, "Source: src\nVersion: 1.0\nArchitecture: amd64\n")?;
        fs::write(&second, "Source: src\nVersion: 1.0\nArchitecture: all\n")?;
        let groups = Group::assemble(&[subject(&first), subject(&second)])?;
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].buildinfo.as_ref().map(|data| data.path.clone()),
            Some(first)
        );
        Ok(())
    }
}
