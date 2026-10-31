//! md5sums control-file checks from lintian's `md5sums` Check.
//!
//! Each Rule owns one Tag. The file text is compared with installed paths and,
//! for a mismatch, the md5 of the file bytes.

mod ln0342_malformed_md5sums_control_file;
mod ln0343_md5sums_lists_nonexistent_file;
mod ln0344_file_missing_in_md5sums;
mod ln0345_md5sum_mismatch;
mod text;

pub(crate) use ln0342_malformed_md5sums_control_file::MalformedMd5sumsControlFile;
pub(crate) use ln0343_md5sums_lists_nonexistent_file::Md5sumsListsNonexistentFile;
pub(crate) use ln0344_file_missing_in_md5sums::FileMissingInMd5sums;
pub(crate) use ln0345_md5sum_mismatch::Md5sumMismatch;
