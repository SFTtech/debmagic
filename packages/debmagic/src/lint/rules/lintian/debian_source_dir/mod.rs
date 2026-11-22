//! The `debian/source-dir` Check's Tags.

mod ln0740_custom_compression_in_debian_source_options;
mod ln0759_missing_debian_source_format;
mod ln0760_git_patches_not_exported;
mod ln1210_unknown_file_in_debian_source;

pub(crate) use ln0740_custom_compression_in_debian_source_options::CustomCompressionInDebianSourceOptions;
pub(crate) use ln0759_missing_debian_source_format::MissingDebianSourceFormat;
pub(crate) use ln0760_git_patches_not_exported::GitPatchesNotExported;
pub(crate) use ln1210_unknown_file_in_debian_source::UnknownFileInDebianSource;
