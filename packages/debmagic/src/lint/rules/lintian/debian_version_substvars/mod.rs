//! Rules for lintian's `debian/version-substvars` Check.
//!
//! `version_substvars` finds the binNMU relation shapes. Each Rule emits one Tag.

mod ln0109_not_binnmuable_all_depends_any;
mod ln0110_not_binnmuable_any_depends_any;
mod ln0111_not_binnmuable_any_depends_all;
mod ln0894_version_substvar_for_external_package;
mod ln1259_substvar_source_version_is_deprecated;
mod version_substvars;

pub(crate) use ln0109_not_binnmuable_all_depends_any::NotBinnmuableAllDependsAny;
pub(crate) use ln0110_not_binnmuable_any_depends_any::NotBinnmuableAnyDependsAny;
pub(crate) use ln0111_not_binnmuable_any_depends_all::NotBinnmuableAnyDependsAll;
pub(crate) use ln0894_version_substvar_for_external_package::VersionSubstvarForExternalPackage;
pub(crate) use ln1259_substvar_source_version_is_deprecated::SubstvarSourceVersionIsDeprecated;
