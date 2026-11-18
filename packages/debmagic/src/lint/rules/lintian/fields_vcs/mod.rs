//! Rules for lintian's `fields/vcs` Check.
//!
//! Each Rule owns one Tag. The Source package's Dsc is read when there is one,
//! otherwise the source paragraph of `debian/control`.

mod ln0797_vcs_field_bitrotted;

pub(crate) use ln0797_vcs_field_bitrotted::VcsFieldBitrotted;
