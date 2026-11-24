use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_text::{self, BadLicense};
use crate::lint::rule::SourceRule;

pub struct LicenseProblemNonFreeRFCBCP78;

declare_rule! {
    /// A Patched file is an RFC that is subject to BCP 78, whose license
    /// restricts its modification and translation.
    ///
    /// The file's cleaned text is matched as lintian's `cruft` Check does: a
    /// keyword, the sentence `bcp 78` and the clause regex. Skipped in a
    /// non-free Source package (see `license_text::is_non_free`). Files whose
    /// basename is on lintian's `cruft/rfc-whitelist` table (and
    /// `debian/copyright`) are exempt. Paths under `.pc/` are skipped. Each
    /// finding has no extra and the path as the Location.
    ///
    /// Differs from lintian: the file is matched whole, not in 16 KiB
    /// sliding-window blocks.
    LicenseProblemNonFreeRFCBCP78,
    code = "LN1270",
    tag = "license-problem-non-free-RFC-BCP78",
    default_selected = true,
    experimental = false,
}

static BCP78: LazyLock<BadLicense> = LazyLock::new(|| {
    BadLicense::new(
        &["license", "document", "bcp", "restriction"],
        &["bcp 78"],
        Some(
            r"this [ ] document [ ] is [ ] subject [ ] to [ ] (?:the [ ] rights [ ]?, [ ] licenses [ ] and [ ]restrictions [ ] contained [ ] in [ ])? bcp [ ] 78",
        ),
    )
});

impl SourceRule for LicenseProblemNonFreeRFCBCP78 {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if license_text::is_non_free(ctx) {
            return;
        }
        let texts = license_text::shared_text_files(ctx);
        let flagged: Vec<_> = texts
            .iter()
            .filter(|file| BCP78.matches(file) && !license_text::rfc_is_exempt(&file.path))
            .map(|file| file.path.clone())
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

    #[test]
    fn source() {
        let pass = vec![
            TestCase::from(vec![("src/main.c", "/* BCP 78 is not mentioned */\n")]),
            TestCase::from(vec![(
                "doc/rfc2629.txt",
                "This document is subject to the rights, licenses and restrictions contained in BCP 78.\n",
            )]),
        ];
        let fail = vec![TestCase::from(vec![(
            "doc/bcp78.txt",
            "This document is subject to the rights, licenses and restrictions contained in BCP 78.\n",
        )])];
        SourceTreeTester::new(LicenseProblemNonFreeRFCBCP78, pass, fail).test_and_snapshot();
    }
}
