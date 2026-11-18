//! Rules for lintian's `fields/vcs` Check.
//!
//! Each Rule owns one Tag. The Source package's Dsc is read when there is one,
//! otherwise the source paragraph of `debian/control`.

mod ln0797_vcs_field_bitrotted;
mod ln0798_vcs_field_mismatch;

pub(crate) use ln0797_vcs_field_bitrotted::VcsFieldBitrotted;
pub(crate) use ln0798_vcs_field_mismatch::VcsFieldMismatch;
