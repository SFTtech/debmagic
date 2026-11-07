//! `README.Debian` text from lintian's `debian/readme` Check.

mod ln0335_readme_debian_mentions_usr_doc;
mod ln0368_readme_debian_contains_invalid_email_address;
mod readme;

pub(crate) use ln0335_readme_debian_mentions_usr_doc::ReadmeDebianMentionsUsrDoc;
pub(crate) use ln0368_readme_debian_contains_invalid_email_address::ReadmeDebianContainsInvalidEmailAddress;
