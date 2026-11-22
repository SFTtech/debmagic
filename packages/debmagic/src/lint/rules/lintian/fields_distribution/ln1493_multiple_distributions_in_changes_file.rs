use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

pub struct MultipleDistributionsInChangesFile;

declare_rule! {
    /// Reports a `.changes` whose `Distribution` names more than one target.
    /// The extra is the words of the field, in order, joined by a space, so
    /// `stable  foo` (any spacing) is `stable foo`. Such a value is valid
    /// syntax, but the Debian archive software does not accept it.
    ///
    /// The words are split on whitespace, as lintian's `trimmed_list` does.
    /// An empty or missing `Distribution` is not reported.
    MultipleDistributionsInChangesFile,
    code = "LN1493",
    tag = "multiple-distributions-in-changes-file",
    default_selected = true,
    experimental = false,
}

impl ChangesRule for MultipleDistributionsInChangesFile {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let distributions = ctx.changes().distributions();
        if distributions.len() > 1 {
            let extra = distributions.join(" ");
            ctx.diagnostic(Diagnostic::error(extra));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester};

    use super::*;

    fn changes(distribution: &str) -> ChangesCase {
        ChangesCase::from(format!(
            "Source: example\nVersion: 1.0\nDistribution: {distribution}\n"
        ))
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            changes("unstable"),
            changes("stable-security"),
            changes(""),
            ChangesCase::from("Source: example\nVersion: 1.0\n"),
        ];
        let fail = vec![
            changes("stable foo"),
            changes("stable   foo\tbar"),
            changes("unstable experimental"),
            // Leading and trailing spaces are not words.
            changes("  sid  bullseye  "),
        ];
        ChangesTester::new(MultipleDistributionsInChangesFile, pass, fail).test_and_snapshot();
    }
}
