use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};
use debmagic_common::compression::gunzip;

use super::content;
use super::manual::is_gzip;

pub struct AcuteAccentInManualPage;

declare_rule! {
    /// A manual page contains a backslash-apostrophe (`\'`).
    ///
    /// Pages are read as in `documentation/manual`: gzip is decompressed, and a
    /// page under 256 bytes whose opening line is empty or a `.so` request is
    /// left alone. Every line is checked, comments included, except a `.ds`
    /// string definition (`^\.\s*ds\s`), which is exempt as in lintian. A page
    /// whose decompressed bytes are not valid UTF-8 is skipped. Each matching
    /// line is reported. Extra is empty; the line is the Location. Udebs are
    /// skipped. Reported at info severity.
    ///
    /// Difference from lintian: lintian first requires the page's file type to
    /// be troff or text. debmagic has no such classification, so any page whose
    /// bytes are valid UTF-8 is checked.
    AcuteAccentInManualPage,
    code = "LN1355",
    tag = "acute-accent-in-manual-page",
    default_selected = false,
    experimental = false,
}

static DS_DEFINITION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\.\s*ds\s").expect("valid regex"));

impl BinaryPackageRule for AcuteAccentInManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for page in content::pages(files) {
            if page.opening.stops() {
                continue;
            }
            let plain = if is_gzip(&page.stored) {
                match gunzip(&page.stored) {
                    Some(plain) => plain,
                    None => continue,
                }
            } else {
                page.stored.clone()
            };
            if std::str::from_utf8(&plain).is_err() {
                continue;
            }
            for (index, line) in page.lines.iter().enumerate() {
                if line.contains("\\'") && !DS_DEFINITION.is_match(line) {
                    hits.push((
                        page.path.clone(),
                        u32::try_from(index + 1).unwrap_or(u32::MAX),
                    ));
                }
            }
        }
        for (path, line) in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: Some(line),
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

    fn page(text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/share/man/man1/example.1", TestFile::new(text))])
            .field("Architecture", "any")
    }

    #[test]
    fn test() {
        let pass = vec![
            page(".TH EXAMPLE 1\n.ds Ac \\'\n"),
            page(".TH EXAMPLE 1\n.SH NAME\nplain text\n"),
            page(".TH EXAMPLE 1\n.SH NAME\n\\(aq\n"),
        ];
        let fail = vec![
            page(".TH EXAMPLE 1\n.SH NAME\ncaf\\'e\n"),
            page(".TH EXAMPLE 1\n.\\\" comment \\'\n.SH NAME\nname \\'x\\'\n"),
        ];
        BinaryPackageTester::new(AcuteAccentInManualPage, pass, fail).test_and_snapshot();
    }
}
