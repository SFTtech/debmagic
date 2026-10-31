//! Rules for the `version=` / `Version:` line of `debian/watch`.

mod ln0089_missing_debian_watch_file_standard;
mod ln0090_obsolete_debian_watch_file_standard;
mod ln0091_unknown_debian_watch_file_standard;
mod ln0092_multiple_debian_watch_file_standards;
mod standard;

pub(crate) use ln0089_missing_debian_watch_file_standard::MissingDebianWatchFileStandard;
pub(crate) use ln0090_obsolete_debian_watch_file_standard::ObsoleteDebianWatchFileStandard;
pub(crate) use ln0091_unknown_debian_watch_file_standard::UnknownDebianWatchFileStandard;
pub(crate) use ln0092_multiple_debian_watch_file_standards::MultipleDebianWatchFileStandards;
