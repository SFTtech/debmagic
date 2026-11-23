use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::spelling::check_spelling_picky;

pub struct CapitalizationInOverrideComment;

declare_rule! {
    /// The `#` comment above an override in the installed override file
    /// spells a term with the wrong capitalization, such as `postgresql` for
    /// `PostgreSQL`.
    ///
    /// Reads `usr/share/lintian/overrides/<package>` (else `.gz`). Each
    /// override with a non-empty justification is checked against lintian's
    /// capitalization list, as its `check_spelling_picky` does; no spelling
    /// exceptions apply. The Location is the line above the override (the last
    /// comment line); extra is the override's Tag, then the wrong spelling and
    /// its correction. Pedantic and not selected by default, as in lintian.
    /// Udebs are checked too.
    CapitalizationInOverrideComment,
    code = "LN1762",
    tag = "capitalization-in-override-comment",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for CapitalizationInOverrideComment {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(overrides) = ctx.lintian_overrides() else {
            return;
        };
        let path = overrides.path().to_path_buf();
        for item in overrides.overrides() {
            let justification = item.justification();
            if justification.is_empty() {
                continue;
            }
            // Comments appear just above the override: point at the line before it.
            let line = item.line().saturating_sub(1);
            for (found, correction) in check_spelling_picky(justification) {
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
            overrides("example: cute-field\n# postgresql\n"),
            overrides("# For PostgreSQL.\nexample: cute-field\n"),
            // Text in square brackets is not checked.
            overrides("# See [postgresql] here.\nexample: cute-field\n"),
        ];
        let fail = vec![
            overrides("# for postgresql\nexample: cute-field\n"),
            overrides("# for postgresql\nexample: cute-field\n").udeb(),
            overrides("# a meta package here\nexample: cute-field\n"),
        ];
        BinaryPackageTester::new(CapitalizationInOverrideComment, pass, fail).test_and_snapshot();
    }
}
