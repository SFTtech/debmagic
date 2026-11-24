//! `conffiles` Check.
//!
//! Each Rule owns one Tag. The control member is the `conffiles` file.

pub(in crate::lint::rules::lintian) mod declared;
mod ln0354_non_etc_file_marked_as_conffile;
mod ln0424_missing_conffile;
mod ln0433_file_in_usr_marked_as_conffile;
mod ln0777_conffile_has_bad_file_type;
mod ln0778_duplicate_conffile;
mod ln0779_file_in_etc_rc_d_marked_as_conffile;
mod ln1012_relative_conffile;
mod ln1013_unexpected_conffile;
mod ln1014_unknown_conffile_instruction;

pub(crate) use ln0354_non_etc_file_marked_as_conffile::NonEtcFileMarkedAsConffile;
pub(crate) use ln0424_missing_conffile::MissingConffile;
pub(crate) use ln0433_file_in_usr_marked_as_conffile::FileInUsrMarkedAsConffile;
pub(crate) use ln0777_conffile_has_bad_file_type::ConffileHasBadFileType;
pub(crate) use ln0778_duplicate_conffile::DuplicateConffile;
pub(crate) use ln0779_file_in_etc_rc_d_marked_as_conffile::FileInEtcRcDMarkedAsConffile;
pub(crate) use ln1012_relative_conffile::RelativeConffile;
pub(crate) use ln1013_unexpected_conffile::UnexpectedConffile;
pub(crate) use ln1014_unknown_conffile_instruction::UnknownConffileInstruction;
