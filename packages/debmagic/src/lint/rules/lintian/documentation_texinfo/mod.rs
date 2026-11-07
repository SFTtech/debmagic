//! Rules for lintian's `documentation/texinfo` Check.
//!
//! Each Rule owns one Tag. `scan` reads gzip info documents.

mod ln0438_info_document_missing_dir_entry;
mod ln0439_info_document_missing_dir_section;
mod scan;

pub(crate) use ln0438_info_document_missing_dir_entry::InfoDocumentMissingDirEntry;
pub(crate) use ln0439_info_document_missing_dir_section::InfoDocumentMissingDirSection;
