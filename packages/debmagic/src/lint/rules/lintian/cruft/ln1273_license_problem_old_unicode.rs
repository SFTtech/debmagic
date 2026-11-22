use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_text::{self, BadLicense};
use crate::lint::rule::SourceRule;

pub struct LicenseProblemOldUnicode;

declare_rule! {
    /// A Patched file carries the old Unicode, Inc. license grant, which
    /// permits using the file's information only to create products
    /// supporting the Unicode Standard, and so is not free.
    ///
    /// The file's cleaned text is matched as lintian's `cruft` Check does: a
    /// keyword, the sentence and the clause regex. Skipped in a non-free
    /// Source package (see `license_text::is_non_free`). Paths under `.pc/`
    /// are skipped. Each finding has no extra and the path as the Location.
    ///
    /// Differs from lintian: the file is matched whole, not in 16 KiB
    /// sliding-window blocks.
    LicenseProblemOldUnicode,
    code = "LN1273",
    tag = "license-problem-old-unicode",
    default_selected = true,
    experimental = false,
}

static OLD_UNICODE: LazyLock<BadLicense> = LazyLock::new(|| {
    BadLicense::new(
        &[
            "unicode",
            "hereby",
            "grants",
            "right",
            "freely",
            "use",
            "information",
            "supplied",
            "file",
            "creation",
            "products",
        ],
        &[
            "unicode, inc. hereby grants the right to freely use the information supplied in this file in the creation of products",
        ],
        Some(
            r"unicode, [ ] inc. [ ] hereby [ ] grants [ ] the [ ] right [ ] to [ ] freely [ ] use [ ] the [ ] information [ ] supplied [ ] in [ ] this [ ] file [ ] in [ ] the [ ] creation [ ] of [ ] products",
        ),
    )
});

impl SourceRule for LicenseProblemOldUnicode {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if license_text::is_non_free(ctx) {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let flagged: Vec<_> = license_text::text_files(files)
            .into_iter()
            .filter(|file| OLD_UNICODE.matches(file))
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

    const CLAUSE: &str = "/*\n * Copyright 2001-2004 Unicode, Inc.\n *\n * Unicode, Inc. hereby grants the right to freely use the information\n * supplied in this file in the creation of products supporting the\n * Unicode Standard.\n */\n";

    #[test]
    fn source() {
        let pass = vec![TestCase::from(vec![(
            "src/main.c",
            "/* Unicode Standard */\n",
        )])];
        let fail = vec![TestCase::from(vec![("src/bad.c", CLAUSE)])];
        SourceTreeTester::new(LicenseProblemOldUnicode, pass, fail).test_and_snapshot();
    }
}
