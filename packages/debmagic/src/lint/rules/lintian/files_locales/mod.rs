//! Rules for lintian's `files/locales` Check.

mod ln1385_incorrect_locale_code;
mod ln1386_unknown_locale_code;
mod locale_folder;

pub(crate) use ln1385_incorrect_locale_code::IncorrectLocaleCode;
pub(crate) use ln1386_unknown_locale_code::UnknownLocaleCode;
