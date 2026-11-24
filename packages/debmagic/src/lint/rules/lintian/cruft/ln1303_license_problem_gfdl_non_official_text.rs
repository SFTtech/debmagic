use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_gfdl::gfdl_hint;
use crate::lint::license_text::{self, text_files};
use crate::lint::rule::{Severity, SourceRule};

pub struct LicenseProblemGfdlNonOfficialText;

declare_rule! {
    /// A Patched file is under the GNU Free Documentation License and words
    /// its "no invariant sections" part in a non-official way. Upstream
    /// should use "with no Invariant Sections, no Front-Cover Texts, and no
    /// Back-Cover Texts". The extra is `invariant part is: <text>`, with
    /// punctuation and line numbers stripped.
    ///
    /// The invariant part is matched as lintian's `cruft` Check does, against
    /// the known-good wordings of `cruft/gfdl-license-fragments-checks` whose
    /// tag is this one. Pedantic and not selected by default, as in lintian.
    /// Skipped in a non-free Source package (see `license_text::is_non_free`),
    /// when the invariant part is empty or the wording is official, and for
    /// paths under `.pc/`. Each finding has the path as the Location.
    ///
    /// Differs from lintian: the file is matched whole, not in 16 KiB
    /// sliding-window blocks.
    LicenseProblemGfdlNonOfficialText,
    code = "LN1303",
    tag = "license-problem-gfdl-non-official-text",
    default_selected = false,
    experimental = false,
}

const TAG: &str = "license-problem-gfdl-non-official-text";

impl SourceRule for LicenseProblemGfdlNonOfficialText {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if license_text::is_non_free(ctx) {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let flagged: Vec<(PathBuf, String)> = text_files(files)
            .into_iter()
            .filter_map(|file| {
                let hint = gfdl_hint(&file)?;
                (hint.tag == TAG).then_some((file.path, hint.extra))
            })
            .collect();
        for (path, extra) in flagged {
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
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    const OFFICIAL: &str = "Permission is granted to copy, distribute and/or modify this document under the terms of the GNU Free Documentation License, Version 1.3 with no Invariant Sections, no Front-Cover Texts, and no Back-Cover Texts. A copy of the license is included.\n";

    const NON_OFFICIAL: &str = "Permission is granted to copy, distribute and/or modify this document under the terms of the GNU Free Documentation License, Version 1.2 with no Invariant Sections, with no Front-Cover Texts, and with no Back-Cover Texts. A copy of the license is included.\n";

    #[test]
    fn source() {
        let pass = vec![
            TestCase::from(vec![("src/main.c", "/* plain source */\n")]),
            TestCase::from(vec![("doc/COPYING", OFFICIAL)]),
        ];
        let fail = vec![TestCase::from(vec![("doc/manual.txt", NON_OFFICIAL)])];
        SourceTreeTester::new(LicenseProblemGfdlNonOfficialText, pass, fail).test_and_snapshot();
    }

    /// Lintian's `full_text_check` matches only its first sliding window
    /// (the first 32 KiB; `last BLOCK` at the loop end), so GFDL wording
    /// that starts past it is not reported.
    #[test]
    fn gfdl_wording_past_first_sliding_window_is_not_matched() {
        let past = format!("{}{NON_OFFICIAL}", "a".repeat(40_000));
        let pass = vec![TestCase::from(vec![("doc/manual.txt", past.as_str())])];
        SourceTreeTester::new(LicenseProblemGfdlNonOfficialText, pass, vec![]).test();
    }

    /// The same wording that ends inside the first 32 KiB is reported.
    #[test]
    fn gfdl_wording_inside_first_sliding_window_is_matched() {
        let inside = format!("{}{NON_OFFICIAL}", "a".repeat(20_000));
        let fail = vec![TestCase::from(vec![("doc/manual.txt", inside.as_str())])];
        SourceTreeTester::new(LicenseProblemGfdlNonOfficialText, vec![], fail).test();
    }
}
