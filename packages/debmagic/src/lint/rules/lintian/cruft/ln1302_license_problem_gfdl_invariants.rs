use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_gfdl::gfdl_hint;
use crate::lint::license_text;
use crate::lint::rule::SourceRule;

pub struct LicenseProblemGfdlInvariants;

declare_rule! {
    /// A Patched file is under the GNU Free Documentation License with
    /// invariant sections, front-cover texts or back-cover texts, which are
    /// not suitable for main. The hint carries the invariant part as
    /// `invariant part is: <text>`, with punctuation and line numbers stripped.
    ///
    /// The file's cleaned text is searched for the license sentence and its
    /// invariant part as lintian's `cruft` Check does. The invariant part is
    /// then compared with the known-good wordings of `cruft/gfdl-license-fragments-checks`
    /// (the official "no invariant sections" wording, the examples of the
    /// license notice, and the fragments accepted for some file names).
    /// Skipped in a non-free Source package (see `license_text::is_non_free`),
    /// when the invariant part is empty, and for paths under `.pc/`. Each
    /// finding has the path as the Location.
    ///
    /// Differs from lintian: the file is matched whole, not in 16 KiB
    /// sliding-window blocks. The fragment tables of the other GFDL Tags
    /// (`license-problem-gfdl-non-official-text` and the empty-invariant
    /// variant) are not reported here.
    LicenseProblemGfdlInvariants,
    code = "LN1302",
    tag = "license-problem-gfdl-invariants",
    default_selected = true,
    experimental = false,
}

const TAG: &str = "license-problem-gfdl-invariants";

impl SourceRule for LicenseProblemGfdlInvariants {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if license_text::is_non_free(ctx) {
            return;
        }
        let texts = license_text::shared_text_files(ctx);
        let flagged: Vec<(PathBuf, String)> = texts
            .iter()
            .filter_map(|file| {
                let hint = gfdl_hint(file)?;
                (hint.tag == TAG).then_some((file.path.clone(), hint.extra))
            })
            .collect();
        for (path, extra) in flagged {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    const OFFICIAL_NOTICE: &str = "Permission is granted to copy, distribute and/or modify this document under the terms of the GNU Free Documentation License, Version 1.3 with no Invariant Sections, no Front-Cover Texts, and no Back-Cover Texts. A copy of the license is included in the section entitled \"GNU Free Documentation License\".\n";

    const INVARIANT_NOTICE: &str = "Permission is granted to copy, distribute and/or modify this document under the terms of the GNU Free Documentation License, Version 1.2 with the Invariant Sections being GNU Manifesto, with the Front-Cover Texts being (A) and with the Back-Cover Texts being (B). A copy of the license is included.\n";

    #[test]
    fn source() {
        let pass = vec![
            TestCase::from(vec![("src/main.c", "/* plain source */\n")]),
            TestCase::from(vec![("doc/COPYING", OFFICIAL_NOTICE)]),
        ];
        let fail = vec![TestCase::from(vec![("doc/manual.txt", INVARIANT_NOTICE)])];
        SourceTreeTester::new(LicenseProblemGfdlInvariants, pass, fail).test_and_snapshot();
    }
}
