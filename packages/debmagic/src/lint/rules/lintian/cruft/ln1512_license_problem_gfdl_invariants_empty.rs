use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_gfdl::gfdl_hint;
use crate::lint::license_text::{self, text_files};
use crate::lint::rule::{Severity, SourceRule};

pub struct LicenseProblemGfdlInvariantsEmpty;

declare_rule! {
    /// A Patched file is under the GNU Free Documentation License but says
    /// nothing about invariant sections, front-cover texts or back-cover
    /// texts: the invariant part is empty once the license's classical words
    /// are removed. The license asks for that absence to be stated. Extra is
    /// empty and the Location is the path.
    ///
    /// The invariant part is matched as lintian's `cruft` Check does (see the
    /// GFDL Rule `license-problem-gfdl-invariants`). Info severity and not
    /// selected by default, as in lintian. Skipped in a non-free Source
    /// package (see `license_text::is_non_free`) and for paths under `.pc/`.
    ///
    /// Differs from lintian: the file is matched whole, not in 16 KiB
    /// sliding-window blocks.
    LicenseProblemGfdlInvariantsEmpty,
    code = "LN1512",
    tag = "license-problem-gfdl-invariants-empty",
    default_selected = false,
    experimental = false,
}

const TAG: &str = "license-problem-gfdl-invariants-empty";

impl SourceRule for LicenseProblemGfdlInvariantsEmpty {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if license_text::is_non_free(ctx) {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let flagged: Vec<PathBuf> = text_files(files)
            .into_iter()
            .filter_map(|file| {
                let hint = gfdl_hint(&file)?;
                (hint.tag == TAG).then_some(file.path)
            })
            .collect();
        for path in flagged {
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

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    const EMPTY: &str = "Permission is granted to copy, distribute and/or modify this\n\
        document under the terms of the GNU Free Documentation License,\n\
        version 1.3 or any later version published by the Free Software\n\
        Foundation. A copy of the license is included in the\n\
        section entitled \"GNU Free Documentation License\".\n";

    const OFFICIAL: &str = "Permission is granted to copy, distribute and/or modify this\n\
        document under the terms of the GNU Free Documentation License,\n\
        version 1.3 or any later version published by the Free Software\n\
        Foundation; with no Invariant Sections, no Front-Cover Texts and\n\
        no Back-Cover Texts.  A copy of the license is included in the\n\
        section entitled \"GNU Free Documentation License\".\n";

    #[test]
    fn source() {
        let pass = vec![
            TestCase::from(vec![("src/main.c", "/* plain source */\n")]),
            // The invariant part is stated, so it is not empty.
            TestCase::from(vec![("doc/COPYING", OFFICIAL)]),
            // Not a GFDL file at all.
            TestCase::from(vec![(
                "doc/notes.txt",
                "A copy of the license is included.\n",
            )]),
        ];
        let fail = vec![
            TestCase::from(vec![("doc/manual.texi", EMPTY)]),
            TestCase::from(vec![("doc/manual.txt", EMPTY), ("doc/COPYING", OFFICIAL)]),
        ];
        SourceTreeTester::new(LicenseProblemGfdlInvariantsEmpty, pass, fail).test_and_snapshot();
    }
}
