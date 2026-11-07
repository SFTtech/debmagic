//! `conffiles` Check.
//!
//! Each Rule owns one Tag. The control member is the `conffiles` file.

mod ln0354_non_etc_file_marked_as_conffile;
mod ln0424_missing_conffile;

pub(crate) use ln0354_non_etc_file_marked_as_conffile::NonEtcFileMarkedAsConffile;
pub(crate) use ln0424_missing_conffile::MissingConffile;
