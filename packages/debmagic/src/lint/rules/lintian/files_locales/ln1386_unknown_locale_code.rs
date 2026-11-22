use std::os::unix::ffi::OsStrExt;

use super::locale_folder::{LocaleFolder, iso_codes};
use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct UnknownLocaleCode;

declare_rule! {
    /// A directory directly under `usr/share/locale/` whose name is not a known
    /// locale code. Reports the directory name, as lintian's `files/locales`
    /// Check does. Only the language part counts: the name is cut at the first
    /// `.` or `@` (encoding, modifier), then at the first `_` (country), and an
    /// empty language is skipped, as is `l10n`.
    ///
    /// Not reported when the language is one of the confusing codes that
    /// `incorrect-locale-code` covers (`al`, `cn`, `cz`, `dk`, `gr`, `in`), when
    /// the country is `en_UK`, or when the language is a known ISO 639 code
    /// (`locale-uses-language-group-code` covers ISO 639-5 group codes).
    ///
    /// Differs from lintian: the code lists are vendored from iso-codes 4.20.1
    /// (`data/files/locale-codes` and `locale-group-codes`) instead of being read
    /// from the host's `/usr/share/iso-codes` at run time.
    UnknownLocaleCode,
    code = "LN1386",
    tag = "unknown-locale-code",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnknownLocaleCode {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let tables = iso_codes();
        let mut reports = Vec::new();
        for entry in files.entries() {
            if !entry.is_dir() {
                continue;
            }
            let Some(folder) = LocaleFolder::parse(entry.path().as_os_str().as_bytes()) else {
                continue;
            };
            if folder.is_confusing()
                || tables.is_locale_code(folder.language)
                || tables.is_group_code(folder.language)
            {
                continue;
            }
            reports.push((
                entry.path().to_path_buf(),
                String::from_utf8_lossy(folder.name).into_owned(),
            ));
        }
        for (path, folder) in reports {
            ctx.diagnostic(Diagnostic::warn(folder).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn dir(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::directory())])
    }

    #[test]
    fn binary() {
        let pass = vec![
            dir("usr/share/locale/de"),
            dir("usr/share/locale/de_DE"),
            dir("usr/share/locale/de.UTF-8"),
            dir("usr/share/locale/sr@latin"),
            dir("usr/share/locale/ind"),
            dir("usr/share/locale/ber"),
            // Confusing codes belong to incorrect-locale-code.
            dir("usr/share/locale/cz_CZ"),
            dir("usr/share/locale/en_UK"),
            // Special case, skipped.
            dir("usr/share/locale/l10n"),
            // Not directly under usr/share/locale.
            dir("usr/share/locale/zz/LC_MESSAGES"),
            // No language before the country.
            dir("usr/share/locale/_zz"),
            // A file named like a locale is not a directory.
            BinaryPackageCase::from(vec![("usr/share/locale/zz", TestFile::new("data"))]),
            // Outside usr/share/locale.
            dir("usr/share/example/zz"),
        ];
        let fail = vec![
            dir("usr/share/locale/zz"),
            dir("usr/share/locale/zz_ZZ"),
            dir("usr/share/locale/xx.UTF-8"),
            // The modifier is cut off, the language is still unknown.
            dir("usr/share/locale/zz@euro"),
        ];
        BinaryPackageTester::new(UnknownLocaleCode, pass, fail).test_and_snapshot();
    }
}
