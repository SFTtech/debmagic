//! `README.Debian` text from lintian's `debian/readme` Check.

mod ln0335_readme_debian_mentions_usr_doc;
mod ln0368_readme_debian_contains_invalid_email_address;
mod ln0893_readme_debian_contains_debmake_template;
mod ln1760_spelling_error_in_readme_debian;
mod readme;

pub(crate) use ln0335_readme_debian_mentions_usr_doc::ReadmeDebianMentionsUsrDoc;
pub(crate) use ln0368_readme_debian_contains_invalid_email_address::ReadmeDebianContainsInvalidEmailAddress;
pub(crate) use ln0893_readme_debian_contains_debmake_template::ReadmeDebianContainsDebmakeTemplate;
pub(crate) use ln1760_spelling_error_in_readme_debian::SpellingErrorInReadmeDebian;
