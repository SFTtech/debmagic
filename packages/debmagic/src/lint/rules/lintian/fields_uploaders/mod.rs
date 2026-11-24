//! Rules for lintian's `fields/uploaders` Check.
//!
//! Each Rule owns one Tag. The Source package's Dsc is read when there is one,
//! otherwise the source paragraph of `debian/control`.

mod ln0776_maintainer_also_in_uploaders;
mod ln1065_uploader_name_missing;

pub(crate) use ln0776_maintainer_also_in_uploaders::MaintainerAlsoInUploaders;
pub(crate) use ln1065_uploader_name_missing::UploaderNameMissing;
