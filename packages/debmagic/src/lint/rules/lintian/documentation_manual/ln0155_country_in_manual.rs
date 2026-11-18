use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::manual::{names_a_country, section_page};

pub struct CountryInManual;

declare_rule! {
    /// A manual-page locale directory names a country.
    ///
    /// The directory between `usr/share/man/` and `manN/` contains `_` and is
    /// not `pt_BR` or `zh_` followed by two ASCII uppercase letters. Files,
    /// hardlinks, and symlinks in that directory are flagged. Udebs are
    /// skipped. Extra is empty; the Data files path is the Location.
    CountryInManual,
    code = "LN0155",
    tag = "country-in-manual",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CountryInManual {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            let Some(page) = section_page(entry) else {
                continue;
            };
            if page.language.as_deref().is_some_and(names_a_country) {
                hits.push(entry.path().to_path_buf());
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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

    fn text(path: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::new("man"))
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn test() {
        let pass = vec![
            case(vec![]),
            case(vec![text("usr/share/man/man1/foo.1.gz")]),
            case(vec![text("usr/share/man/de/man1/foo.1.gz")]),
            case(vec![text("usr/share/man/pt_BR/man1/foo.1.gz")]),
            case(vec![
                text("usr/share/man/zh_CN/man1/foo.1.gz"),
                text("usr/share/man/zh_TW/man1/foo.1.gz"),
                text("usr/share/man/zh_HK/man1/foo.1.gz"),
            ]),
            // Not a section directory, so the locale is never read.
            case(vec![text("usr/share/man/de_DE/test-utf8-chars.1p.gz")]),
            case(vec![text("usr/share/man/de_DE/man1/test-utf8-chars.1p.gz")]).udeb(),
        ];
        let fail = vec![
            case(vec![text("usr/share/man/de_DE/man1/test-utf8-chars.1p.gz")]),
            case(vec![
                text("usr/share/man/en_US/man1/foo.1.gz"),
                text("usr/share/man/pt_PT/man1/foo.1.gz"),
                text("usr/share/man/zh_cn/man1/foo.1.gz"),
                text("usr/share/man/zh_Hant/man1/foo.1.gz"),
            ]),
            case(vec![(
                "usr/share/man/fr_FR/man1/foo.1.gz".to_string(),
                TestFile::symlink("foo.1.gz"),
            )]),
            case(vec![text("usr/share/man/de_DE/man1/plain.1")]),
        ];
        BinaryPackageTester::new(CountryInManual, pass, fail).test_and_snapshot();
    }
}
