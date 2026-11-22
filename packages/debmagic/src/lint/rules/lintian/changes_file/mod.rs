//! Rules for lintian's `changes-file` Check.

mod ln1440_bad_section_in_changes_file;
mod ln1490_file_size_mismatch_in_changes_file;

pub(crate) use ln1440_bad_section_in_changes_file::BadSectionInChangesFile;
pub(crate) use ln1490_file_size_mismatch_in_changes_file::FileSizeMismatchInChangesFile;
