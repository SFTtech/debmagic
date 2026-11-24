use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_text::{self, BadLicense};
use crate::lint::rule::SourceRule;

pub struct LicenseProblemNvidiaIntellectual;

declare_rule! {
    /// A Patched file carries the NVIDIA OpenCV license clause that reserves
    /// all intellectual property and proprietary rights, so the file may
    /// not be distributed.
    ///
    /// The file's cleaned text is matched as lintian's `cruft` Check does: a
    /// keyword and the sentence, with no regex. Unlike the other license
    /// problems, this applies in non-free Source packages too. Paths under
    /// `.pc/` are skipped. Each finding has no extra and the path as the
    /// Location.
    ///
    /// Differs from lintian: the file is matched whole, not in 16 KiB
    /// sliding-window blocks.
    LicenseProblemNvidiaIntellectual,
    code = "LN1272",
    tag = "license-problem-nvidia-intellectual",
    default_selected = true,
    experimental = false,
}

static NVIDIA: LazyLock<BadLicense> = LazyLock::new(|| {
    BadLicense::new(
        &["license", "intellectual", "retain", "property"],
        &[
            "retain all intellectual property and proprietary rights in and to this software and related documentation",
        ],
        None,
    )
});

impl SourceRule for LicenseProblemNvidiaIntellectual {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let texts = license_text::shared_text_files(ctx);
        let flagged: Vec<_> = texts
            .iter()
            .filter(|file| NVIDIA.matches(file))
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

    const CLAUSE: &str = "/*\n * Acme Corporation and its licensors retain all intellectual property and\n * proprietary rights in and to this software and related documentation.\n */\n";

    #[test]
    fn source() {
        let pass = vec![
            TestCase::from(vec![("src/main.c", "/* MIT license */\n")]),
            TestCase::from(vec![(
                "src/main.c",
                "/* retain all intellectual property */\n",
            )]),
        ];
        let fail = vec![
            TestCase::from(vec![("src/badcv.c", CLAUSE)]),
            TestCase::from(vec![("debian/copyright", CLAUSE)]),
        ];
        SourceTreeTester::new(LicenseProblemNvidiaIntellectual, pass, fail).test_and_snapshot();
    }
}
