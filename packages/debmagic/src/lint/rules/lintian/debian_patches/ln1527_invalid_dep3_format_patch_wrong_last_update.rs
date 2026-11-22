use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep3_header::dep3_headers;

pub struct InvalidDep3FormatPatchWrongLastUpdate;

declare_rule! {
    /// A DEP-3 `Last-Update` field is not a `YYYY-MM-DD` date that exists.
    ///
    /// Reads each patch under `debian/patches/` except `series` and `README`
    /// by its header, the text before the first line starting with `---`. Fires
    /// when `Last-Update` is set and is not four, two and two digits forming a
    /// calendar date in or after 1900, so `2021-02-29`, `1899-12-31` and
    /// `2020-1-01` fire. Extra is empty. Location is the patch. Info, not
    /// selected by default. A patch without Description or Subject, or without
    /// Origin, Author or From, is skipped. Lintian round-trips the date through
    /// the local time zone; this Rule checks the calendar alone, so a midnight
    /// that does not exist in the local zone is accepted here.
    InvalidDep3FormatPatchWrongLastUpdate,
    code = "LN1527",
    tag = "invalid-dep3-format-patch-wrong-last-update",
    default_selected = false,
    experimental = false,
}

impl SourceRule for InvalidDep3FormatPatchWrongLastUpdate {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, header) in dep3_headers(ctx) {
            if header.last_update_is_wrong() {
                ctx.diagnostic(
                    Diagnostic::warn("")
                        .with_severity(Severity::Info)
                        .with_location(Location {
                            path,
                            line: None,
                            column: None,
                        }),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn patch(header: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/patches/fix.patch",
            TestFile::new(format!("{header}---\n\n--- a/foo\n+++ b/foo\n")),
        )]
    }

    const GATE: &str = "Description: fix\nOrigin: upstream, commit:0123abcd\n";

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            patch(GATE),
            patch(&format!("{GATE}Last-Update:\n")),
            patch(&format!("{GATE}Last-Update: 2020-02-29\n")),
            patch(&format!("{GATE}Last-Update: 2000-02-29\n")),
            patch(&format!("{GATE}Last-Update: 1900-01-01\n")),
            patch(&format!("{GATE}Last-Update: 2024-12-31\n")),
            // Without Description and Subject the patch is skipped.
            patch("Origin: upstream, commit:0123abcd\nLast-Update: 2021-02-29\n"),
            // The last mention counts.
            patch(&format!(
                "{GATE}Last-Update: 2021-02-29\nLast-Update: 2021-03-01\n"
            )),
            // Field names are case-insensitive.
            patch("description: fix\nOrigin: upstream, commit:0123abcd\nlast-update: 2021-03-01\n"),
            vec![(
                "debian/patches/series",
                TestFile::new("Last-Update: 2021-02-29\n"),
            )],
        ];
        let fail = vec![
            patch(&format!("{GATE}Last-Update: 2021-02-29\n")),
            patch(&format!("{GATE}Last-Update: 1899-12-31\n")),
            patch(&format!("{GATE}Last-Update: 2020-13-01\n")),
            patch(&format!("{GATE}Last-Update: 2020-00-10\n")),
            patch(&format!("{GATE}Last-Update: 2020-04-31\n")),
            patch(&format!("{GATE}Last-Update: 2020-01-00\n")),
            patch(&format!("{GATE}Last-Update: 2020-1-01\n")),
            patch(&format!("{GATE}Last-Update: 2020/01/01\n")),
            patch(&format!("{GATE}Last-Update: 2020-01-01T00:00:00\n")),
            vec![
                (
                    "debian/patches/one.patch",
                    TestFile::new(format!("{GATE}Last-Update: 2021-02-29\n---\n")),
                ),
                (
                    "debian/patches/two.patch",
                    TestFile::new(format!("{GATE}Last-Update: 2021-02-30\n---\n")),
                ),
            ],
        ];
        SourceTreeTester::new(InvalidDep3FormatPatchWrongLastUpdate, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(patch(&format!(
            "{GATE}Last-Update: 2020-02-29\n"
        )))];
        let fail = vec![SourcePackageCase::from(patch(&format!(
            "{GATE}Last-Update: 2021-02-29\n"
        )))];
        SourcePackageTester::new(InvalidDep3FormatPatchWrongLastUpdate, pass, fail)
            .test_and_snapshot();
    }
}
