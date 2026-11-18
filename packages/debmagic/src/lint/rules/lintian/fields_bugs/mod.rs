//! Rules for lintian's `fields/bugs` Check.
//!
//! Each Rule owns one Tag.

mod ln0801_bugs_field_does_not_refer_to_debian_infrastructure;

pub(crate) use ln0801_bugs_field_does_not_refer_to_debian_infrastructure::BugsFieldDoesNotReferToDebianInfrastructure;
