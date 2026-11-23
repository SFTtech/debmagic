use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::spelling::{check_spelling, group_exceptions};

use super::copyright_text;

pub struct SpellingErrorInCopyright;

declare_rule! {
    /// The installed copyright file contains a misspelling from lintian's
    /// spelling lists.
    ///
    /// Checks the text of the installed copyright file (after gunzipping a
    /// `copyright.gz`, and following a symlink; files that are not UTF-8 are
    /// skipped) with the vendored `spelling/corrections` and
    /// `spelling/corrections-multiword` lists. Exceptions are the group
    /// exceptions of the Binary package (its names and people). Extra is the
    /// misspelling and its correction; the Location is empty. Repeated words
    /// are not checked. Pedantic, not selected by default. Udebs are not
    /// checked.
    SpellingErrorInCopyright,
    code = "LN1643",
    tag = "spelling-error-in-copyright",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for SpellingErrorInCopyright {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        let exceptions = group_exceptions(ctx);
        for (found, correction) in check_spelling(&contents, &exceptions, false) {
            ctx.diagnostic(
                Diagnostic::warn(format!("{found} {correction}")).with_severity(Severity::Pedantic),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn installed(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("usr/share/doc/example", TestFile::directory()),
            ("usr/share/doc/example/copyright", TestFile::new(body)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(installed("Copyright 2024 Someone\n")),
            BinaryPackageCase::from(installed("unknow\n")).udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed("Copyright 2024 Someone\nunknow\n")),
            BinaryPackageCase::from(installed("This is deafult text.\n")),
        ];
        BinaryPackageTester::new(SpellingErrorInCopyright, pass, fail).test_and_snapshot();
    }
}
