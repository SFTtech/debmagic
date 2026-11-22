use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::content;

pub struct PythonTracebackInManpage;

declare_rule! {
    /// A manual page contains a Python traceback.
    ///
    /// Pages are read as in `documentation/manual`: gzip is decompressed, and a
    /// page under 256 bytes whose opening line is empty or a `.so` request is
    /// left alone. `.\"` comment lines are skipped. Only the first
    /// `Traceback (most recent call last):` on a page is reported, with the
    /// page as the Location and no line. Udebs are skipped. Extra is empty.
    /// Reported at error severity.
    PythonTracebackInManpage,
    code = "LN1356",
    tag = "python-traceback-in-manpage",
    default_selected = true,
    experimental = false,
}

static TRACEBACK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Traceback \(most recent call last\):").expect("valid regex"));

impl BinaryPackageRule for PythonTracebackInManpage {
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
            let found = page
                .lines
                .iter()
                .filter(|line| !line.starts_with(r#".\""#))
                .any(|line| TRACEBACK.is_match(line));
            if found {
                hits.push(page.path.clone());
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
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

    fn page(text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/share/man/man1/example.1", TestFile::new(text))])
            .field("Architecture", "any")
    }

    #[test]
    fn test() {
        let pass = vec![
            page(".TH EXAMPLE 1\n.\\\" Traceback (most recent call last):\n"),
            page(".TH EXAMPLE 1\n.SH NAME\ntraceback in prose\n"),
        ];
        let fail = vec![
            page(".TH EXAMPLE 1\n.SH NAME\nTraceback (most recent call last):\n"),
            page(
                ".TH EXAMPLE 1\nTraceback (most recent call last):\nTraceback (most recent call last):\n",
            ),
        ];
        BinaryPackageTester::new(PythonTracebackInManpage, pass, fail).test_and_snapshot();
    }
}
