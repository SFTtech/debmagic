//! Rules for lintian's `debian/control/field/build-profiles` Check.

mod ln1017_invalid_profile_name_in_build_profiles_field;
mod ln1018_varies_content_with_nocheck;

pub(crate) use ln1017_invalid_profile_name_in_build_profiles_field::InvalidProfileNameInBuildProfilesField;
pub(crate) use ln1018_varies_content_with_nocheck::VariesContentWithNocheck;
