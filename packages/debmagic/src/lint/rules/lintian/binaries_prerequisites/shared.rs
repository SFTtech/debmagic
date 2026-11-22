//! Facts the `binaries/prerequisites` Checks read in common: the ELF
//! executables and shared objects of a Binary package, and its strong
//! relations.

use std::path::PathBuf;

use crate::lint::context::BinaryPackageContext;
use crate::lint::elf::ElfObjectType;
use crate::lint::file_tree::EntryKind;
use crate::lint::package_relation::PackageRelation;

/// An ELF program or shared object of the Binary package, with its `NEEDED`
/// entries in dynamic-section order.
pub(super) struct Object {
    pub(super) path: PathBuf,
    pub(super) shared: bool,
    pub(super) needed: Vec<String>,
}

/// The ELF executables (`ET_EXEC` or PIE) and shared objects (`ET_DYN`
/// without `DF_1_PIE`) among the Data files that are regular files or
/// hardlinks. Relocatable objects are not included, as lintian's
/// `executable | shared object` test excludes them. Sorted by path.
pub(super) fn objects(ctx: &mut BinaryPackageContext<'_>) -> Vec<Object> {
    let Some(elf_files) = ctx.data_elf_files() else {
        return Vec::new();
    };
    let Some(files) = ctx.data_files() else {
        return Vec::new();
    };
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter_map(|entry| {
            let facts = elf_files.get(entry.path())?;
            let shared = match facts.object_type() {
                ElfObjectType::Executable => false,
                ElfObjectType::SharedObject => true,
                ElfObjectType::Relocatable | ElfObjectType::Other => return None,
            };
            Some(Object {
                path: entry.path().to_path_buf(),
                shared,
                needed: facts.needed().to_vec(),
            })
        })
        .collect()
}

/// `Pre-Depends` and `Depends` AND-ed: lintian's `relation('strong')`.
pub(super) fn strong(ctx: &BinaryPackageContext<'_>) -> PackageRelation {
    ctx.installation_control()
        .fields()
        .relations_and(&["Pre-Depends", "Depends"])
}

/// No alternative at all: lintian's `is_empty`.
pub(super) fn is_empty(relation: &PackageRelation) -> bool {
    relation.alternatives().next().is_none()
}

/// lintian's `matches(qr/^<package>\b/)` on a relation: an alternative's
/// package name starts with `package`, and the name does not continue with a
/// word character.
pub(super) fn names_package_prefix(relation: &PackageRelation, package: &str) -> bool {
    relation.alternatives().any(|alternative| {
        alternative
            .package()
            .strip_prefix(package)
            .is_some_and(|rest| !rest.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
    })
}

/// The file names of `ELF` objects with `NEEDED` entries, sorted and unique.
pub(super) fn sorted_unique(names: &[PathBuf]) -> Vec<String> {
    let mut names: Vec<String> = names
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.dedup();
    names
}
