use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::spelling::{check_spelling, group_exceptions};

use super::content;

pub struct TypoInManualPage;

declare_rule! {
    /// An English manual page has a line with a common misspelling.
    ///
    /// Lintian's `documentation/manual` Check. A regular file or hardlink in
    /// `usr/share/man/.../manN/` (the English hierarchy, not a language
    /// subdirectory) is read as the other page Rules read it: gzip is
    /// decompressed, and a page under 256 bytes whose opening line is empty or
    /// a `.so` request is left alone. Each line is checked with lintian's
    /// `check_spelling` against the vendored `spelling/corrections` and
    /// `spelling/corrections-multiword` tables, with the spelling exceptions of
    /// the Binary package's group, and `.\"` comment lines are not checked.
    /// Repeated words are not reported here (duplicate checking is off, as in
    /// lintian). Extra is `<word> <correction>` (or the quoted phrase pair for
    /// multi-word misspellings). The Location is the page and the line number,
    /// counting comment lines. Udebs are skipped.
    TypoInManualPage,
    code = "LN1542",
    tag = "typo-in-manual-page",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for TypoInManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let exceptions = group_exceptions(ctx);
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for page in content::pages(files) {
            if page.opening.stops() || !page.english_hierarchy {
                continue;
            }
            for (index, line) in page.lines.iter().enumerate() {
                if line.starts_with(r#".\""#) {
                    continue;
                }
                for (found, correction) in check_spelling(line, &exceptions, false) {
                    hits.push((
                        page.path.clone(),
                        (index + 1) as u32,
                        format!("{found} {correction}"),
                    ));
                }
            }
        }
        for (path, line, message) in hits {
            ctx.diagnostic(
                Diagnostic::warn(message)
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

    fn page(path: &str, text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(text))]).field("Architecture", "any")
    }

    #[test]
    fn test() {
        let english = "usr/share/man/man1/example.1";
        let pass = vec![
            page(english, ".TH EXAMPLE 1\n.SH NAME\nexample - a tool\n"),
            // Comment lines are not checked.
            page(
                english,
                ".TH EXAMPLE 1\n.\\\" recieve the input\n.SH NAME\nexample\n",
            ),
            // Other language directories are not the English hierarchy.
            page(
                "usr/share/man/de/man1/example.1",
                ".TH EXAMPLE 1\n.SH NAME\nrecieve\n",
            ),
            // Short opening `.so` request is left alone.
            page(english, ".so man1/other.1\n"),
            // Udebs have no manual pages.
            page(english, ".TH EXAMPLE 1\n.SH NAME\nrecieve it\n").udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            page(
                english,
                ".TH EXAMPLE 1\n.SH NAME\nexample - recieve the input\n",
            ),
            page(
                english,
                ".TH EXAMPLE 1\n.\\\" a comment\n.SH NAME\nrecieve it\n",
            ),
        ];
        BinaryPackageTester::new(TypoInManualPage, pass, fail).test_and_snapshot();
    }
}
