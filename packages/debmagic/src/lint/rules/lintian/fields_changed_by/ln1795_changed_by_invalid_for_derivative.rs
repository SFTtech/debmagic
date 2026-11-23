use crate::declare_rule;
use crate::lint::derivative;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

pub struct ChangedByInvalidForDerivative;

declare_rule! {
    /// The `Changed-By` field of the `.changes` does not match one of the
    /// derivative patterns of lintian's `data/common/derivative-changed-by`.
    /// The extra is the value and the pattern's explanation, in brackets; one
    /// hint for each pattern it does not match.
    ///
    /// Debian's table has one pattern, `.`, which matches any value with at
    /// least one character. So the Rule reports a `Changed-By` with an empty
    /// value (`Changed-By:` alone), with the explanation `always matches`, and
    /// nothing else. A `.changes` without the field is not checked. Lintian
    /// reads the value trimmed; this Rule does too.
    ChangedByInvalidForDerivative,
    code = "LN1795",
    tag = "changed-by-invalid-for-derivative",
    default_selected = true,
    experimental = false,
}

impl ChangesRule for ChangedByInvalidForDerivative {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let Some(changed_by) = ctx.changes().changed_by().map(str::to_string) else {
            return;
        };
        for pattern in derivative::changed_by() {
            if !pattern.regex.is_match(&changed_by) {
                ctx.diagnostic(Diagnostic::error(format!(
                    "{changed_by} ({})",
                    pattern.explanation
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester};

    use super::*;

    fn changes(changed_by: Option<&str>) -> ChangesCase {
        let mut text = String::from("Source: example\nVersion: 1.0\n");
        if let Some(value) = changed_by {
            text.push_str(&format!("Changed-By:{value}\n"));
        }
        ChangesCase::from(text)
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            changes(None),
            changes(Some(" Alice <alice@example.com>")),
            changes(Some(" x")),
        ];
        let fail = vec![changes(Some(""))];
        ChangesTester::new(ChangedByInvalidForDerivative, pass, fail).test_and_snapshot();
    }
}
