//! Shared library facts for the `libraries/shared/links` Rules: which installed
//! files are shared libraries in an ldconfig folder, and the names lintian
//! derives from their SONAME.

use std::path::PathBuf;

use crate::lint::elf::ElfFiles;
use crate::lint::file_tree::FileTree;
use crate::lint::rules::lintian::fields_relations_more::architectures::is_ldconfig_folder;

/// Every regular file or hardlink with a `DT_SONAME` in an ldconfig folder,
/// with the versioned link name lintian derives: its directory and SONAME.
/// Files at the top of the tree have no such folder and are left out.
pub(super) fn shared_libraries(files: &FileTree, elf: &ElfFiles) -> Vec<(PathBuf, String)> {
    files
        .entries()
        .iter()
        .filter(|entry| entry.is_file())
        .filter_map(|entry| {
            let path = entry.path();
            let soname = elf.get(path)?.soname().first()?;
            let dir = path.parent()?.to_str()?;
            if dir.is_empty() || !is_ldconfig_folder(dir) {
                return None;
            }
            Some((path.to_path_buf(), format!("{dir}/{soname}")))
        })
        .collect()
}
