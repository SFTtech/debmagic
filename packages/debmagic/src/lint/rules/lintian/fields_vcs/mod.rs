//! Rules for lintian's `fields/vcs` Check.
//!
//! Each Rule owns one Tag. The Source package's Dsc is read when there is one,
//! otherwise the source paragraph of `debian/control`.

mod ln0797_vcs_field_bitrotted;
mod ln0798_vcs_field_mismatch;
mod ln0799_orphaned_package_maintained_in_private_space;

pub(crate) use ln0797_vcs_field_bitrotted::VcsFieldBitrotted;
pub(crate) use ln0798_vcs_field_mismatch::VcsFieldMismatch;
pub(crate) use ln0799_orphaned_package_maintained_in_private_space::OrphanedPackageMaintainedInPrivateSpace;
