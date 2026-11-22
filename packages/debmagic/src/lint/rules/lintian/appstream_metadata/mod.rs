//! Rules for lintian's `appstream-metadata` Check.

mod ln1462_appstream_metadata_in_legacy_location;
mod ln1463_appstream_metadata_invalid;
mod ln1464_appstream_metadata_legacy_format;

pub(crate) use ln1462_appstream_metadata_in_legacy_location::AppstreamMetadataInLegacyLocation;
pub(crate) use ln1463_appstream_metadata_invalid::AppstreamMetadataInvalid;
pub(crate) use ln1464_appstream_metadata_legacy_format::AppstreamMetadataLegacyFormat;
