//! Rules for lintian's `archive/liberty` Check.
//!
//! Each Rule owns one Tag.

mod ln0389_archive_liberty_mismatch;

pub(crate) use ln0389_archive_liberty_mismatch::ArchiveLibertyMismatch;
