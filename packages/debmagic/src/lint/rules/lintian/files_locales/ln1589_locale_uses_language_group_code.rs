use std::os::unix::ffi::OsStrExt;

use super::locale_folder::{LocaleFolder, iso_codes};
use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct LocaleUsesLanguageGroupCode;

declare_rule! {
    /// A directory directly under `usr/share/locale/` named after an ISO 639-5
    /// language group code (`ber`, `cus`, `art`, ...), which is not a language
    /// locale code. Reported at pedantic severity with the folder name as the
    /// extra, as lintian's `files/locales` Check does.
    ///
    /// Not reported when the language is also a locale code, or is one of the
    /// confusing codes that `incorrect-locale-code` covers. The folder is split as
    /// lintian's Check splits it; `l10n` and an empty language are skipped.
    ///
    /// Differs from lintian: the code lists are vendored from iso-codes 4.20.1
    /// (`data/files/locale-codes` and `locale-group-codes`) instead of being read
    /// from the host's `/usr/share/iso-codes` at run time.
    LocaleUsesLanguageGroupCode,
    code = "LN1589",
    tag = "locale-uses-language-group-code",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for LocaleUsesLanguageGroupCode {
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
                || !tables.is_group_code(folder.language)
            {
                continue;
            }
            reports.push((
                entry.path().to_path_buf(),
                String::from_utf8_lossy(folder.name).into_owned(),
            ));
        }
        for (path, extra) in reports {
            ctx.diagnostic(
                Diagnostic::warn(extra)
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
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
            dir("usr/share/locale/ind"),
            dir("usr/share/locale/en_GB.UTF-8"),
            // Confusing codes belong to incorrect-locale-code.
            dir("usr/share/locale/cz_CZ"),
            // Unknown codes belong to unknown-locale-code.
            dir("usr/share/locale/zz"),
            // Special case, skipped.
            dir("usr/share/locale/l10n"),
            // Not directly under usr/share/locale.
            dir("usr/share/example/aav"),
            // A file named like a group code is not a directory.
            BinaryPackageCase::from(vec![("usr/share/locale/aav", TestFile::new("data"))]),
        ];
        let fail = vec![
            dir("usr/share/locale/aav"),
            dir("usr/share/locale/ber"),
            dir("usr/share/locale/cus.UTF-8"),
            dir("usr/share/locale/art_XX@euro"),
        ];
        BinaryPackageTester::new(LocaleUsesLanguageGroupCode, pass, fail).test_and_snapshot();
    }
}
