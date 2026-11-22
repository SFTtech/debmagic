use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_text::{self, BadLicense};
use crate::lint::rule::SourceRule;

pub struct LicenseProblemNonFreeRFC;

declare_rule! {
    /// A Patched file is an RFC whose license forbids modifying the document
    /// in any way, the older RFC wording that is not free.
    ///
    /// The file's cleaned text is matched as lintian's `cruft` Check does: a
    /// keyword, the sentence and the clause regex. Skipped in a non-free
    /// Source package (see `license_text::is_non_free`). Files whose basename
    /// is on lintian's `cruft/rfc-whitelist` table (and `debian/copyright`)
    /// are exempt. Paths under `.pc/` are skipped. Each finding has no extra
    /// and the path as the Location.
    ///
    /// Differs from lintian: the file is matched whole, not in 16 KiB
    /// sliding-window blocks.
    LicenseProblemNonFreeRFC,
    code = "LN1271",
    tag = "license-problem-non-free-RFC",
    default_selected = true,
    experimental = false,
}

static RFC: LazyLock<BadLicense> = LazyLock::new(|| {
    BadLicense::new(
        &["document", "purpose", "translate", "language"],
        &["this document itself may not be modified in any way"],
        Some(
            r"this [ ] document [ ] itself [ ] may [ ] not [ ] be [ ] modified [ ] in [ ] any [ ] way [ ]?,
            [ ]? such [ ] as [ ] by [ ] removing [ ] the [ ] copyright [ ] notice [ ] or [ ] references
            [ ] to [ ] .{0,256} [ ]? except [ ] as [ ] needed [ ] for [ ] the [ ] purpose [ ] of [ ] developing
            [ ] .{0,128} [ ]? in [ ] which [ ] case [ ] the [ ] procedures [ ] for [ ] copyrights [ ] defined
            [ ] in [ ] the [ ] .{0,128} [ ]? process [ ] must [ ] be [ ] followed[ ]?,[ ]?
            or [ ] as [ ] required [ ] to [ ] translate [ ] it [ ] into [ ] languages [ ]",
        ),
    )
});

impl SourceRule for LicenseProblemNonFreeRFC {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if license_text::is_non_free(ctx) {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let flagged: Vec<_> = license_text::text_files(files)
            .into_iter()
            .filter(|file| RFC.matches(file) && !license_text::rfc_is_exempt(&file.path))
            .map(|file| file.path)
            .collect();
        for path in flagged {
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
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    const CLAUSE: &str = "This document itself may not be modified in any way, such as by removing the copyright notice or references to the Internet Society or other Internet organizations, except as needed for the purpose of developing Internet standards in which case the procedures for copyrights defined in the Internet Standards process must be followed, or as required to translate it into languages other than English.\n";

    #[test]
    fn source() {
        let pass = vec![
            TestCase::from(vec![("src/main.c", "/* Internet standards */\n")]),
            TestCase::from(vec![("doc/rfc2629.txt", CLAUSE)]),
        ];
        let fail = vec![TestCase::from(vec![("doc/rfc.txt", CLAUSE)])];
        SourceTreeTester::new(LicenseProblemNonFreeRFC, pass, fail).test_and_snapshot();
    }
}
