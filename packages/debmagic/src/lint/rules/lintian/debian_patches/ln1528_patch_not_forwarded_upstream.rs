use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep3_header::dep3_headers;

pub struct PatchNotForwardedUpstream;

declare_rule! {
    /// A patch whose DEP-3 header does not say it was forwarded upstream.
    ///
    /// Reads each patch under `debian/patches/` except `series` and `README`
    /// by its header, the text before the first line starting with `---`. Fires
    /// when Origin does not start with `upstream` or `backport`, and either
    /// Forwarded is `no`, or none of Applied-Upstream, Bug and Forwarded is set.
    /// Extra is empty. Location is the patch. Info, not selected by default. A
    /// patch without Description or Subject, or without Origin, Author or From,
    /// is skipped.
    PatchNotForwardedUpstream,
    code = "LN1528",
    tag = "patch-not-forwarded-upstream",
    default_selected = false,
    experimental = false,
}

impl SourceRule for PatchNotForwardedUpstream {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, header) in dep3_headers(ctx) {
            if header.not_forwarded_upstream() {
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

    const GATE: &str = "Description: fix\n";

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            patch(&format!("{GATE}Origin: upstream, commit:0123abcd\n")),
            patch(&format!("{GATE}Origin: backport, https://example.org/1\n")),
            // Any of Applied-Upstream, Bug and Forwarded satisfies the other half.
            patch(&format!(
                "{GATE}Origin: vendor, https://example.org/1\nForwarded: yes\n"
            )),
            patch(&format!(
                "{GATE}Origin: vendor, https://example.org/1\nBug: https://example.org/b\n"
            )),
            patch(&format!(
                "{GATE}Origin: vendor, https://example.org/1\nApplied-Upstream: 1.2\n"
            )),
            // Forwarded set to anything but `no` is accepted.
            patch(&format!(
                "{GATE}Origin: other, https://example.org/1\nForwarded: not-needed\n"
            )),
            // Without Description and Subject the patch is skipped.
            patch("Origin: vendor, https://example.org/1\n"),
            // The last mention counts.
            patch(&format!(
                "{GATE}Origin: vendor, x\nForwarded: no\nForwarded: yes\n"
            )),
            // Field names are case-insensitive, and the Origin category is trimmed.
            patch("description: fix\nORIGIN: upstream , commit:0123abcd\n"),
            vec![(
                "debian/patches/series",
                TestFile::new("Origin: vendor, x\n"),
            )],
        ];
        let fail = vec![
            patch(&format!("{GATE}Origin: vendor, https://example.org/1\n")),
            patch(&format!(
                "{GATE}Origin: other, https://example.org/1\nForwarded: no\n"
            )),
            // An empty Forwarded does not count as set.
            patch(&format!("{GATE}Origin: vendor, x\nForwarded:\n")),
            // Forwarded: no fires even with a Bug.
            patch(&format!(
                "{GATE}Origin: vendor, x\nBug: https://example.org/b\nForwarded: no\n"
            )),
            // A Subject and an Author satisfy the gates.
            patch("Subject: fix\nAuthor: a\nOrigin: vendor, x\n"),
            vec![
                (
                    "debian/patches/one.patch",
                    TestFile::new(format!("{GATE}Origin: vendor, x\n---\n")),
                ),
                (
                    "debian/patches/two.patch",
                    TestFile::new(format!("{GATE}From: a\nForwarded: no\n---\n")),
                ),
            ],
        ];
        SourceTreeTester::new(PatchNotForwardedUpstream, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(patch(&format!(
            "{GATE}Origin: upstream, commit:0123abcd\n"
        )))];
        let fail = vec![SourcePackageCase::from(patch(&format!(
            "{GATE}Origin: vendor, x\n"
        )))];
        SourcePackageTester::new(PatchNotForwardedUpstream, pass, fail).test_and_snapshot();
    }
}
