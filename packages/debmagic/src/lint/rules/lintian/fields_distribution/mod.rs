//! Rules for lintian's `fields/distribution` Check.

mod ln1493_multiple_distributions_in_changes_file;
mod ln1496_unreleased_changes;

pub(crate) use ln1493_multiple_distributions_in_changes_file::MultipleDistributionsInChangesFile;
pub(crate) use ln1496_unreleased_changes::UnreleasedChanges;
