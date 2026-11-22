//! Rules for lintian's `fields/format` Check.

mod ln1052_unsupported_source_format;
mod ln1492_malformed_changes_file;

pub(crate) use ln1052_unsupported_source_format::UnsupportedSourceFormat;
pub(crate) use ln1492_malformed_changes_file::MalformedChangesFile;
