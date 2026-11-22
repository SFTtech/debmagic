//! Rules for lintian's `appstream-metadata` Check.

mod ln1462_appstream_metadata_in_legacy_location;
mod ln1463_appstream_metadata_invalid;

pub(crate) use ln1462_appstream_metadata_in_legacy_location::AppstreamMetadataInLegacyLocation;
pub(crate) use ln1463_appstream_metadata_invalid::AppstreamMetadataInvalid;
