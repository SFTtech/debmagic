//! Rules for lintian's `images/filenames` Check.

mod ln1395_image_file_has_conflicting_name;
mod ln1396_image_file_has_unexpected_name;

pub(crate) use ln1395_image_file_has_conflicting_name::ImageFileHasConflictingName;
pub(crate) use ln1396_image_file_has_unexpected_name::ImageFileHasUnexpectedName;
