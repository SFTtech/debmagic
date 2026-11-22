//! Rules for lintian's `files/architecture` Check.

mod ln1066_triplet_dir_and_architecture_mismatch;
mod ln1374_package_contains_no_arch_dependent_files;

pub(crate) use ln1066_triplet_dir_and_architecture_mismatch::TripletDirAndArchitectureMismatch;
pub(crate) use ln1374_package_contains_no_arch_dependent_files::PackageContainsNoArchDependentFiles;
