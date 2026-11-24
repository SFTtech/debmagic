use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::spelling::{check_spelling, group_exceptions};

pub struct SpellingInOverrideComment;

declare_rule! {
    /// The `#` comment above an override in the installed override file
    /// contains a misspelling from lintian's spelling lists.
    ///
    /// Reads `usr/share/lintian/overrides/<package>` (else `.gz`). Each
    /// override with a non-empty justification is checked with the spelling
    /// exceptions of the Binary package's group, as lintian's `check_spelling`
    /// does. The Location is the line above the override (the last comment
    /// line); extra is the override's Tag, then the word and its correction.
    /// Pedantic and not selected by default, as in lintian. Udebs are checked
    /// too.
    SpellingInOverrideComment,
    code = "LN1761",
    tag = "spelling-in-override-comment",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for SpellingInOverrideComment {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(overrides) = ctx.lintian_overrides() else {
            return;
        };
        let path = overrides.path().to_path_buf();
        let exceptions = group_exceptions(ctx);
        for item in overrides.overrides() {
            let justification = item.justification();
            if justification.is_empty() {
                continue;
            }
            // Comments appear just above the override: point at the line before it.
            let line = item.line().saturating_sub(1);
            for (found, correction) in check_spelling(justification, &exceptions, true) {
                ctx.diagnostic(
                    Diagnostic::warn(format!("{} {found} {correction}", item.tag()))
                        .with_location(Location {
                            path: path.clone(),
                            line: Some(line),
                            column: None,
                        })
                        .with_severity(Severity::Pedantic),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn overrides(text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/lintian/overrides/example".to_string(),
            TestFile::new(text),
        )])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            overrides("example: cute-field\n"),
            // Without a justification nothing is checked.
            overrides("example: cute-field\n# teh speling\n"),
            overrides("# A correct comment.\nexample: cute-field\n"),
            // Exceptions of the package name are not reported.
            overrides("# example is fine\nexample: cute-field\n"),
            overrides("# A correct comment.\n\nexample: cute-field\n"),
        ];
        let fail = vec![
            overrides("# absolutly no context\nexample: cute-field\n"),
            overrides("# Old speling.\nexample: cute-field\n").udeb(),
            overrides("# A word word here.\nexample: cute-field\n"),
        ];
        BinaryPackageTester::new(SpellingInOverrideComment, pass, fail).test_and_snapshot();
    }
}
