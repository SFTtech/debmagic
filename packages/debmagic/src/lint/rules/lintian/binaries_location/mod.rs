//! `binaries/location`: an architecture-specific file of a Multi-Arch: same Binary package.

mod ln0722_arch_dependent_file_not_in_arch_specific_directory;
mod ln0737_arch_dependent_file_in_usr_share;

pub(crate) use ln0722_arch_dependent_file_not_in_arch_specific_directory::ArchDependentFileNotInArchSpecificDirectory;
pub(crate) use ln0737_arch_dependent_file_in_usr_share::ArchDependentFileInUsrShare;
