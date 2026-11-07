//! Rules for lintian's `debian/changelog` and `nmu` Checks.
//!
//! Source Tags read `debian/changelog` entries and parse epoch, upstream, and
//! revision inside this module. NMU Tags also compare the trailer with
//! `debian/control`. Binary Tags read the installed changelog or `NEWS.Debian.gz`
//! from Data files and parse them with [`DebianChangelog`](crate::lint::debian_changelog::DebianChangelog).

mod installed;
mod ln0306_epoch_changed_but_upstream_version_did_not_go_backwards;
mod ln0307_epoch_change_without_comment;
mod ln0308_latest_debian_changelog_entry_reuses_existing_version;
mod ln0309_odd_historical_debian_changelog_version;
mod ln0310_debian_changelog_has_wrong_day_of_week;
mod ln0311_debian_changelog_line_too_long;
mod ln0312_initial_upload_closes_no_bugs;
mod ln0313_no_nmu_in_changelog;
mod ln0314_source_nmu_has_incorrect_version_number;
mod ln0315_no_qa_in_changelog;
mod ln0361_bogus_mail_host_in_debian_changelog;
mod ln0362_syntax_error_in_debian_news_file;
mod ln0363_latest_changelog_entry_without_new_date;
mod ln0364_debian_news_entry_has_strange_distribution;
mod ln0365_wrong_name_for_debian_news_file;
mod ln0366_qa_upload_has_incorrect_version_number;
mod ln0380_changelog_news_debian_mismatch;
mod ln0410_uploaders_in_orphan;
mod ln0441_invalid_date_in_debian_changelog;
mod nmu;
mod source;
mod version;

pub(crate) use ln0306_epoch_changed_but_upstream_version_did_not_go_backwards::EpochChangedButUpstreamVersionDidNotGoBackwards;
pub(crate) use ln0307_epoch_change_without_comment::EpochChangeWithoutComment;
pub(crate) use ln0308_latest_debian_changelog_entry_reuses_existing_version::LatestDebianChangelogEntryReusesExistingVersion;
pub(crate) use ln0309_odd_historical_debian_changelog_version::OddHistoricalDebianChangelogVersion;
pub(crate) use ln0310_debian_changelog_has_wrong_day_of_week::DebianChangelogHasWrongDayOfWeek;
pub(crate) use ln0311_debian_changelog_line_too_long::DebianChangelogLineTooLong;
pub(crate) use ln0312_initial_upload_closes_no_bugs::InitialUploadClosesNoBugs;
pub(crate) use ln0313_no_nmu_in_changelog::NoNmuInChangelog;
pub(crate) use ln0314_source_nmu_has_incorrect_version_number::SourceNmuHasIncorrectVersionNumber;
pub(crate) use ln0315_no_qa_in_changelog::NoQaInChangelog;
pub(crate) use ln0361_bogus_mail_host_in_debian_changelog::BogusMailHostInDebianChangelog;
pub(crate) use ln0362_syntax_error_in_debian_news_file::SyntaxErrorInDebianNewsFile;
pub(crate) use ln0363_latest_changelog_entry_without_new_date::LatestChangelogEntryWithoutNewDate;
pub(crate) use ln0364_debian_news_entry_has_strange_distribution::DebianNewsEntryHasStrangeDistribution;
pub(crate) use ln0365_wrong_name_for_debian_news_file::WrongNameForDebianNewsFile;
pub(crate) use ln0366_qa_upload_has_incorrect_version_number::QaUploadHasIncorrectVersionNumber;
pub(crate) use ln0380_changelog_news_debian_mismatch::ChangelogNewsDebianMismatch;
pub(crate) use ln0410_uploaders_in_orphan::UploadersInOrphan;
pub(crate) use ln0441_invalid_date_in_debian_changelog::InvalidDateInDebianChangelog;
