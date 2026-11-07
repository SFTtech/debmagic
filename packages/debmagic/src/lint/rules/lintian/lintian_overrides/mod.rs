//! Rules for lintian's `debian/lintian-overrides` Check that look at installed paths.

mod ln0427_override_file_in_wrong_location;

pub(crate) use ln0427_override_file_in_wrong_location::OverrideFileInWrongLocation;
