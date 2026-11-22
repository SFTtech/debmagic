//! Shared by the `files/locales` Rules: the folder names under `usr/share/locale/`
//! and the iso-codes tables they are checked against.
//!
//! The tables are vendored from iso-codes 4.20.1 by `scripts/vendor_iso_codes.py`
//! (the headers of the data files record the version), not read from the host.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crate::lint::data;

/// lintian's `%CONFUSING_LANGUAGES`: codes that are not the ISO 639 code, with the
/// code it means.
const CONFUSING_LANGUAGES: [(&[u8], &[u8]); 6] = [
    (b"al", b"sq"),
    (b"cn", b"zh"),
    (b"cz", b"cs"),
    (b"dk", b"da"),
    (b"gr", b"el"),
    (b"in", b"id"),
];

/// lintian's `%CONFUSING_COUNTRIES`: folder names with a wrong country, with the
/// right one.
const CONFUSING_COUNTRIES: [(&[u8], &[u8]); 1] = [(b"en_UK", b"en_GB")];

const LOCALE_CODES: &str = include_str!("../../../data/files/locale-codes");
const LOCALE_GROUP_CODES: &str = include_str!("../../../data/files/locale-group-codes");
const PREFERRED_CODES: &str = include_str!("../../../data/files/locale-preferred-codes");

/// A folder directly under `usr/share/locale/`, split as lintian's Check splits it.
pub(super) struct LocaleFolder<'a> {
    /// The folder name, as in the archive.
    pub(super) name: &'a [u8],
    /// The name without encoding or modifier (`de_DE.UTF-8` gives `de_DE`).
    pub(super) with_country: &'a [u8],
    /// The language before the first `_` (`de` for `de_DE`).
    pub(super) language: &'a [u8],
}

impl<'a> LocaleFolder<'a> {
    /// The folder under `usr/share/locale/` that `path` names, if any. `None` for
    /// paths elsewhere, nested paths, an empty language and lintian's `l10n`.
    pub(super) fn parse(path: &'a [u8]) -> Option<Self> {
        let name = path.strip_prefix(b"usr/share/locale/")?;
        if name.is_empty() || name.contains(&b'/') {
            return None;
        }
        let with_country = name
            .split(|&byte| byte == b'.' || byte == b'@')
            .next()
            .unwrap_or_default();
        if with_country == b"l10n" {
            return None;
        }
        let language = with_country
            .split(|&byte| byte == b'_')
            .next()
            .unwrap_or_default();
        if language.is_empty() {
            return None;
        }
        Some(Self {
            name,
            with_country,
            language,
        })
    }

    /// Whether the language or country is one of lintian's confusing codes, which
    /// `incorrect-locale-code` reports and `unknown-locale-code` skips.
    pub(super) fn is_confusing(&self) -> bool {
        CONFUSING_LANGUAGES
            .iter()
            .any(|(key, _)| *key == self.language)
            || CONFUSING_COUNTRIES
                .iter()
                .any(|(key, _)| *key == self.with_country)
    }

    /// The folder name with lintian's confusing language or country corrected, if
    /// the folder is one of them.
    #[allow(dead_code)] // Used by incorrect-locale-code (LN1385), added next.
    pub(super) fn confusing_fix(&self) -> Option<Vec<u8>> {
        if let Some((_, fix)) = CONFUSING_LANGUAGES
            .iter()
            .find(|(key, _)| *key == self.language)
        {
            return Some(replace_prefix(self.name, self.language, fix));
        }
        let (_, fix) = CONFUSING_COUNTRIES
            .iter()
            .find(|(key, _)| *key == self.with_country)?;
        Some(replace_prefix(self.name, self.with_country, fix))
    }
}

fn replace_prefix(name: &[u8], prefix: &[u8], replacement: &[u8]) -> Vec<u8> {
    let mut fixed = replacement.to_vec();
    fixed.extend_from_slice(&name[prefix.len()..]);
    fixed
}

/// The iso-codes tables, read once on first use.
pub(super) struct IsoCodes {
    /// ISO 639 codes, alpha_2 and alpha_3, of the entries lintian's `LOCALE_CODES`
    /// is built from.
    codes: HashSet<&'static str>,
    /// ISO 639-5 group codes (`LOCALE_GROUP_CODES`).
    group_codes: HashSet<&'static str>,
    /// alpha_3 to alpha_2 for the codes that have one (`LOCALE_CODES` values).
    preferred: HashMap<&'static str, &'static str>,
}

impl IsoCodes {
    /// Whether `code` is one of lintian's `LOCALE_CODES` keys.
    pub(super) fn is_locale_code(&self, code: &[u8]) -> bool {
        std::str::from_utf8(code).is_ok_and(|code| self.codes.contains(code))
    }

    /// Whether `code` is one of lintian's `LOCALE_GROUP_CODES` keys.
    pub(super) fn is_group_code(&self, code: &[u8]) -> bool {
        std::str::from_utf8(code).is_ok_and(|code| self.group_codes.contains(code))
    }

    /// The two-letter code lintian prefers for a three-letter `code`, if it has one.
    #[allow(dead_code)] // Used by incorrect-locale-code (LN1385), added next.
    pub(super) fn preferred_code(&self, code: &[u8]) -> Option<&'static str> {
        let code = std::str::from_utf8(code).ok()?;
        self.preferred.get(code).copied()
    }
}

/// The vendored iso-codes tables.
pub(super) fn iso_codes() -> &'static IsoCodes {
    static TABLES: OnceLock<IsoCodes> = OnceLock::new();
    TABLES.get_or_init(|| IsoCodes {
        codes: data::lines(LOCALE_CODES).collect(),
        group_codes: data::lines(LOCALE_GROUP_CODES).collect(),
        preferred: data::lines(PREFERRED_CODES)
            .filter_map(|line| line.split_once(' '))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_splits_like_lintian() {
        let folder = LocaleFolder::parse(b"usr/share/locale/de_DE.UTF-8").unwrap();
        assert_eq!(folder.name, b"de_DE.UTF-8");
        assert_eq!(folder.with_country, b"de_DE");
        assert_eq!(folder.language, b"de");

        let modifier = LocaleFolder::parse(b"usr/share/locale/sr@latin").unwrap();
        assert_eq!(modifier.with_country, b"sr");
    }

    #[test]
    fn parse_skips_what_lintian_skips() {
        assert!(LocaleFolder::parse(b"usr/share/locale/l10n").is_none());
        assert!(LocaleFolder::parse(b"usr/share/locale/_zz").is_none());
        assert!(LocaleFolder::parse(b"usr/share/locale/zz/LC_MESSAGES").is_none());
        assert!(LocaleFolder::parse(b"usr/share/locale/").is_none());
        assert!(LocaleFolder::parse(b"usr/share/example/zz").is_none());
    }

    #[test]
    fn confusing_fixes() {
        let language = LocaleFolder::parse(b"usr/share/locale/cz_CZ.UTF-8").unwrap();
        assert!(language.is_confusing());
        assert_eq!(language.confusing_fix().unwrap(), b"cs_CZ.UTF-8");

        let country = LocaleFolder::parse(b"usr/share/locale/en_UK").unwrap();
        assert!(country.is_confusing());
        assert_eq!(country.confusing_fix().unwrap(), b"en_GB");

        let plain = LocaleFolder::parse(b"usr/share/locale/de").unwrap();
        assert!(!plain.is_confusing());
        assert!(plain.confusing_fix().is_none());
    }

    #[test]
    fn tables_are_read_from_the_vendored_files() {
        let tables = iso_codes();
        // ISO 639-3 alpha_3 and alpha_2 both are locale codes.
        assert!(tables.is_locale_code(b"ind"));
        assert!(tables.is_locale_code(b"id"));
        assert!(!tables.is_locale_code(b"zz"));
        // ISO 639-5 group codes are not locale codes.
        assert!(tables.is_group_code(b"aav"));
        assert!(!tables.is_locale_code(b"aav"));
        assert!(!tables.is_group_code(b"ind"));
    }

    #[test]
    fn preferred_codes_map_three_letters_to_two() {
        let tables = iso_codes();
        assert_eq!(tables.preferred_code(b"ind"), Some("id"));
        assert_eq!(tables.preferred_code(b"eng"), Some("en"));
        // A two-letter code is already preferred, and some codes have no two-letter code.
        assert_eq!(tables.preferred_code(b"id"), None);
        assert_eq!(tables.preferred_code(b"aaa"), None);
    }
}
