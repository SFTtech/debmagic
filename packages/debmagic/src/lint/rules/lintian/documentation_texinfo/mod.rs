//! Rules for lintian's `documentation/texinfo` Check.
//!
//! Each Rule owns one Tag. `scan` reads gzip info documents.

mod ln0438_info_document_missing_dir_entry;
mod ln0439_info_document_missing_dir_section;
mod ln0747_info_document_has_wrong_extension;
mod ln0748_info_document_not_compressed;
mod scan;

pub(crate) use ln0438_info_document_missing_dir_entry::InfoDocumentMissingDirEntry;
pub(crate) use ln0439_info_document_missing_dir_section::InfoDocumentMissingDirSection;
pub(crate) use ln0747_info_document_has_wrong_extension::InfoDocumentHasWrongExtension;
pub(crate) use ln0748_info_document_not_compressed::InfoDocumentNotCompressed;
