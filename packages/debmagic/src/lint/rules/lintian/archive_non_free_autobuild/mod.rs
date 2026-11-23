//! Rules for lintian's `archive/non-free/autobuild` Check.

mod ln1510_source_only_upload_to_non_free_without_autobuild;

pub(crate) use ln1510_source_only_upload_to_non_free_without_autobuild::SourceOnlyUploadToNonFreeWithoutAutobuild;
