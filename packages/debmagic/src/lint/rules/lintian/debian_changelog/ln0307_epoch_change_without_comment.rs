use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::source::{changelog_location, epoch_arrow, epoch_delta};

pub struct EpochChangeWithoutComment;

declare_rule! {
    /// The epoch changed and the newest changelog entry never says "epoch".
    ///
    /// Compares the newest `debian/changelog` entry with the one before it,
    /// native packages included. The word is a case-insensitive whole word in
    /// that entry's changes. Extra is `previous -> latest`. The newest heading
    /// is the Location.
    EpochChangeWithoutComment,
    code = "LN0307",
    tag = "epoch-change-without-comment",
    default_selected = true,
    experimental = false,
}

impl SourceRule for EpochChangeWithoutComment {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(delta) = epoch_delta(ctx) else {
            return;
        };
        if delta.mentions_epoch {
            return;
        }
        ctx.diagnostic(
            Diagnostic::warn(epoch_arrow(&delta)).with_location(changelog_location(delta.line)),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(format: &str, changelog: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/source/format", TestFile::new(format)),
            ("debian/changelog", TestFile::new(changelog)),
        ]
    }

    fn entry(version: &str, body: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * {body}\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("2:1.0-1", "The epoch changed on purpose."),
                    entry("1:1.0-1", "Previous.")
                ),
            ),
            tree("3.0 (quilt)\n", &entry("1.0-1", "No epoch here.")),
        ];
        let fail = vec![
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("2:1.0-1", "Bumped the version number."),
                    entry("1:1.0-1", "Previous.")
                ),
            ),
            // "epochs" is not the word epoch.
            tree(
                "3.0 (native)\n",
                &format!(
                    "{}{}",
                    entry("2:1.0", "Several epochs of history."),
                    entry("1.0", "Previous.")
                ),
            ),
            tree(
                "3.0 (native)\n",
                &format!(
                    "{}{}",
                    entry("1:1.0", "Introduces a version prefix."),
                    entry("999999", "Previous.")
                ),
            ),
        ];
        SourceTreeTester::new(EpochChangeWithoutComment, pass, fail).test_and_snapshot();
    }
}
