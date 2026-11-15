//! Rules for lintian's `maintainer-scripts/diversion` Check.
//!
//! Each Rule reads the Binary package's Diversions
//! (`BinaryPackageContext::diversions`) and owns one Tag.

mod ln0392_package_uses_local_diversion;
mod ln0709_diversion_for_unknown_file;

pub(crate) use ln0392_package_uses_local_diversion::PackageUsesLocalDiversion;
pub(crate) use ln0709_diversion_for_unknown_file::DiversionForUnknownFile;
