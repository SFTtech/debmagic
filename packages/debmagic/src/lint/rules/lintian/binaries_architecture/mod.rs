//! Rules for lintian's `binaries/architecture` Check.

mod ln1005_arch_independent_package_contains_binary_or_object;
mod ln1291_binary_from_other_architecture;

pub(crate) use ln1005_arch_independent_package_contains_binary_or_object::ArchIndependentPackageContainsBinaryOrObject;
pub(crate) use ln1291_binary_from_other_architecture::BinaryFromOtherArchitecture;
