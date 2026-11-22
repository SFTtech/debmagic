use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

pub struct MalformedChangesFile;

declare_rule! {
    /// Reports a `.changes` without a `Format` field, or with an empty one.
    /// Such a file is probably not a changes file at all, or is not in the
    /// Deb822 format, or is signed twice. The hint has an empty extra.
    ///
    /// Lintian reads the field unfolded; a value that is only whitespace is
    /// empty there, and is empty here too, since the parsed value is trimmed.
    /// A `.changes` that does not parse at all is not reported by this Rule:
    /// the Group has no `.changes` to run it on.
    MalformedChangesFile,
    code = "LN1492",
    tag = "malformed-changes-file",
    default_selected = true,
    experimental = false,
}

impl ChangesRule for MalformedChangesFile {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        if ctx.changes().format().unwrap_or("").is_empty() {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester};

    use super::*;

    #[test]
    fn changes_file() {
        let pass = vec![
            ChangesCase::from("Format: 1.8\nSource: example\nVersion: 1.0\n"),
            ChangesCase::from("Format: 1.8\n"),
        ];
        let fail = vec![
            ChangesCase::from("Source: example\nVersion: 1.0\n"),
            // Only whitespace after the colon.
            ChangesCase::from("Format:   \nSource: example\nVersion: 1.0\n"),
            // A Format field in a later paragraph does not count.
            ChangesCase::from("Source: example\n\nFormat: 1.8\n"),
        ];
        ChangesTester::new(MalformedChangesFile, pass, fail).test_and_snapshot();
    }
}
