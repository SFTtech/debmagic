//! Rules for lintian's `documentation/texinfo` Check.
//!
//! Each Rule owns one Tag. `scan` reads gzip info documents.

mod ln0438_info_document_missing_dir_entry;
mod ln0439_info_document_missing_dir_section;
mod ln0747_info_document_has_wrong_extension;
mod ln0748_info_document_not_compressed;
mod ln0749_info_document_not_compressed_with_gzip;
mod ln0750_info_document_not_compressed_with_max_compression;
mod scan;

pub(crate) use ln0438_info_document_missing_dir_entry::InfoDocumentMissingDirEntry;
pub(crate) use ln0439_info_document_missing_dir_section::InfoDocumentMissingDirSection;
pub(crate) use ln0747_info_document_has_wrong_extension::InfoDocumentHasWrongExtension;
pub(crate) use ln0748_info_document_not_compressed::InfoDocumentNotCompressed;
pub(crate) use ln0749_info_document_not_compressed_with_gzip::InfoDocumentNotCompressedWithGzip;
pub(crate) use ln0750_info_document_not_compressed_with_max_compression::InfoDocumentNotCompressedWithMaxCompression;
