use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_text::{self, BadLicense};
use crate::lint::rule::SourceRule;

pub struct LicenseProblemJsonEvil;

declare_rule! {
    /// A Patched file carries the "software shall be used for good, not evil"
    /// clause of the JSON license, which is not free.
    ///
    /// The file's cleaned text is matched as lintian's `cruft` Check does: a
    /// keyword, the sentence and the clause regex. Skipped in a non-free
    /// Source package (see `license_text::is_non_free`). Paths under `.pc/`
    /// are skipped. Each finding has no extra and the path as the Location.
    ///
    /// Differs from lintian: the file is matched whole, not in 16 KiB
    /// sliding-window blocks.
    LicenseProblemJsonEvil,
    code = "LN1269",
    tag = "license-problem-json-evil",
    default_selected = true,
    experimental = false,
}

static JSON_EVIL: LazyLock<BadLicense> = LazyLock::new(|| {
    BadLicense::new(
        &["software", "evil", "good"],
        &["software shall be used for good"],
        Some(r"software [ ] shall [ ] be [ ] used [ ] for [ ] good [ ]? ,? [ ]? not [ ] evil"),
    )
});

impl SourceRule for LicenseProblemJsonEvil {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if license_text::is_non_free(ctx) {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let flagged: Vec<_> = license_text::text_files(files)
            .into_iter()
            .filter(|file| JSON_EVIL.matches(file))
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

    #[test]
    fn source() {
        let pass = vec![
            TestCase::from(vec![("src/main.c", "/* good software */\n")]),
            TestCase::from(vec![(
                "src/main.c",
                "/* The software shall be used for good */\n",
            )]),
        ];
        let fail = vec![TestCase::from(vec![(
            "src/main.c",
            " * The software shall be used for good, not evil\n",
        )])];
        SourceTreeTester::new(LicenseProblemJsonEvil, pass, fail).test_and_snapshot();
    }
}
