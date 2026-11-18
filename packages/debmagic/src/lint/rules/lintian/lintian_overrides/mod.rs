//! Rules for lintian's `debian/lintian-overrides` Checks: installed paths and
//! the parsed Lintian override file.

mod ln0427_override_file_in_wrong_location;
mod ln0707_alien_tag;
mod ln0839_malformed_override;
mod ln0840_duplicate_override_context;
mod ln0841_override_file_in_wrong_package;

pub(crate) use ln0427_override_file_in_wrong_location::OverrideFileInWrongLocation;
pub(crate) use ln0707_alien_tag::AlienTag;
pub(crate) use ln0839_malformed_override::MalformedOverride;
pub(crate) use ln0840_duplicate_override_context::DuplicateOverrideContext;
pub(crate) use ln0841_override_file_in_wrong_package::OverrideFileInWrongPackage;
