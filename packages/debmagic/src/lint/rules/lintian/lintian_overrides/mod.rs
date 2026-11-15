//! Rules for lintian's `debian/lintian-overrides` Checks: installed paths and
//! the parsed Lintian override file.

mod ln0427_override_file_in_wrong_location;
mod ln0707_alien_tag;

pub(crate) use ln0427_override_file_in_wrong_location::OverrideFileInWrongLocation;
pub(crate) use ln0707_alien_tag::AlienTag;
