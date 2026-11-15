//! The `debian/patches/quilt` Check's Tags.

mod ln0396_format_3_0_but_debian_changes_patch;
mod ln0706_patch_file_present_but_not_mentioned_in_series;

pub(crate) use ln0396_format_3_0_but_debian_changes_patch::Format30ButDebianChangesPatch;
pub(crate) use ln0706_patch_file_present_but_not_mentioned_in_series::PatchFilePresentButNotMentionedInSeries;
