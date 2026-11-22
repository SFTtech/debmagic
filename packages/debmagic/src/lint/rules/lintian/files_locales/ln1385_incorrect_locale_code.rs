use std::os::unix::ffi::OsStrExt;

use super::locale_folder::{LocaleFolder, iso_codes, replace_prefix};
use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct IncorrectLocaleCode;

declare_rule! {
    /// A directory directly under `usr/share/locale/` whose language or country is
    /// not the one lintian prefers, reported as `folder -> fixed`. Covers lintian's
    /// confusing language codes (`al`, `cn`, `cz`, `dk`, `gr`, `in`), the country
    /// `en_UK`, and three-letter ISO 639 codes that have a two-letter code (`ind`
    /// gives `id`). The folder is split as lintian's Check does: cut at the first
    /// `.` or `@`, then at the first `_`; an empty language and `l10n` are skipped.
    ///
    /// Differs from lintian: the code tables are vendored from iso-codes 4.20.1
    /// (`data/files/locale-preferred-codes`) instead of being read from the host's
    /// `/usr/share/iso-codes` at run time.
    IncorrectLocaleCode,
    code = "LN1385",
    tag = "incorrect-locale-code",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for IncorrectLocaleCode {
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
            let fixed = match folder.confusing_fix() {
                Some(fixed) => fixed,
                None => {
                    let Some(preferred) = tables.preferred_code(folder.language) else {
                        continue;
                    };
                    replace_prefix(folder.name, folder.language, preferred.as_bytes())
                }
            };
            reports.push((
                entry.path().to_path_buf(),
                format!(
                    "{} -> {}",
                    String::from_utf8_lossy(folder.name),
                    String::from_utf8_lossy(&fixed),
                ),
            ));
        }
        for (path, extra) in reports {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
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
            dir("usr/share/locale/de_DE.UTF-8"),
            dir("usr/share/locale/en_GB"),
            // Three-letter code without a two-letter code.
            dir("usr/share/locale/ber"),
            // Special case, skipped.
            dir("usr/share/locale/l10n"),
            // A file, not a directory.
            BinaryPackageCase::from(vec![("usr/share/locale/deu", TestFile::new("data"))]),
        ];
        let fail = vec![
            dir("usr/share/locale/ind"),
            dir("usr/share/locale/deu_DE"),
            dir("usr/share/locale/eng@latin"),
            dir("usr/share/locale/cz_CZ"),
            dir("usr/share/locale/en_UK"),
            dir("usr/share/locale/en_UK.UTF-8"),
            dir("usr/share/locale/gr"),
        ];
        BinaryPackageTester::new(IncorrectLocaleCode, pass, fail).test_and_snapshot();
    }
}
