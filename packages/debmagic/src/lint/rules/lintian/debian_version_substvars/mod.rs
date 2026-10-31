//! Rules for lintian's `debian/version-substvars` Check.
//!
//! `version_substvars` finds the binNMU relation shapes. Each Rule emits one Tag.

mod ln0109_not_binnmuable_all_depends_any;
mod ln0110_not_binnmuable_any_depends_any;
mod ln0111_not_binnmuable_any_depends_all;
mod version_substvars;

pub(crate) use ln0109_not_binnmuable_all_depends_any::NotBinnmuableAllDependsAny;
pub(crate) use ln0110_not_binnmuable_any_depends_any::NotBinnmuableAnyDependsAny;
pub(crate) use ln0111_not_binnmuable_any_depends_all::NotBinnmuableAnyDependsAll;
