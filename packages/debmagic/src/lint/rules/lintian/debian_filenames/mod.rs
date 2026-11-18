//! Rules for lintian's `debian/filenames` Check.
//!
//! Reads the packaging File tree of a Source tree or Source package. Each Rule
//! emits one Tag.

mod ln0803_duplicate_packaging_file;
mod misnamed;

pub(crate) use ln0803_duplicate_packaging_file::DuplicatePackagingFile;
