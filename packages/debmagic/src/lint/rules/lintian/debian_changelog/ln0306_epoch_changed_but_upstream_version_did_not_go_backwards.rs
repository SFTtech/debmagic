use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::source::{changelog_location, epoch_arrow, epoch_delta};

pub struct EpochChangedButUpstreamVersionDidNotGoBackwards;

declare_rule! {
    /// The epoch changed and the upstream version did not sort as older.
    ///
    /// Compares the newest `debian/changelog` entry with the one before it.
    /// Fires when their epochs differ, the package is not native, and dpkg
    /// does not order the previous upstream version strictly after the newest.
    /// Extra is `previous -> latest`. The newest heading is the Location.
    EpochChangedButUpstreamVersionDidNotGoBackwards,
    code = "LN0306",
    tag = "epoch-changed-but-upstream-version-did-not-go-backwards",
    default_selected = true,
    experimental = false,
}

impl SourceRule for EpochChangedButUpstreamVersionDidNotGoBackwards {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(delta) = epoch_delta(ctx) else {
            return;
        };
        if delta.native || delta.upstream_went_backwards {
            return;
        }
        ctx.diagnostic(
            Diagnostic::error(epoch_arrow(&delta)).with_location(changelog_location(delta.line)),
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
            tree("3.0 (quilt)\n", &entry("1.0-1", "Same epoch.")),
            // Upstream really went backwards.
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("2:1.0-1", "Epoch, upstream decreased."),
                    entry("1:2.0-1", "Newer upstream.")
                ),
            ),
            // Native packages are exempt even when the epoch changes.
            tree(
                "3.0 (native)\n",
                &format!(
                    "{}{}",
                    entry("2:1.0", "Epoch on a native package."),
                    entry("1:2.0", "Previous.")
                ),
            ),
        ];
        let fail = vec![
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("1:1.0-1", "Added an epoch."),
                    entry("1.0-1", "Previous.")
                ),
            ),
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("2:1.0-3", "Epoch and a higher upstream."),
                    entry("1:0.9-4", "Previous.")
                ),
            ),
        ];
        SourceTreeTester::new(EpochChangedButUpstreamVersionDidNotGoBackwards, pass, fail)
            .test_and_snapshot();
    }
}
