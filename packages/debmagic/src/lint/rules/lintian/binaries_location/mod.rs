//! `binaries/location`: an architecture-specific file of a Multi-Arch: same Binary package.

mod ln0722_arch_dependent_file_not_in_arch_specific_directory;
mod ln0737_arch_dependent_file_in_usr_share;
mod ln0855_binary_in_etc;
mod ln1294_development_package_ships_elf_binary_in_path;

pub(crate) use ln0722_arch_dependent_file_not_in_arch_specific_directory::ArchDependentFileNotInArchSpecificDirectory;
pub(crate) use ln0737_arch_dependent_file_in_usr_share::ArchDependentFileInUsrShare;
pub(crate) use ln0855_binary_in_etc::BinaryInEtc;
pub(crate) use ln1294_development_package_ships_elf_binary_in_path::DevelopmentPackageShipsElfBinaryInPath;
