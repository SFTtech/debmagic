use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::source::{changelog_location, odd_historical};

pub struct OddHistoricalDebianChangelogVersion;

declare_rule! {
    /// The changelog entry before the newest has a version that does not
    /// parse for the package's current nativeness.
    ///
    /// Only that second entry is checked. A native package rejects a Debian
    /// revision; a non-native package rejects a version with no revision.
    /// Extra is `version (for native)` or `version (for non-native)`. That
    /// entry's heading is the Location.
    OddHistoricalDebianChangelogVersion,
    code = "LN0309",
    tag = "odd-historical-debian-changelog-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for OddHistoricalDebianChangelogVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((line, extra)) = odd_historical(ctx) else {
            return;
        };
        ctx.diagnostic(Diagnostic::warn(extra).with_location(changelog_location(line)));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn entry(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(format: &str, changelog: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/source/format", TestFile::new(format)),
            ("debian/changelog", TestFile::new(changelog)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("3.0 (quilt)\n", &entry("1.0-1")),
            tree(
                "3.0 (quilt)\n",
                &format!("{}{}", entry("1.0-2"), entry("1.0-1")),
            ),
            tree(
                "3.0 (native)\n",
                &format!("{}{}", entry("1.0"), entry("0.9")),
            ),
            // Only the second entry is judged; an older odd version is quiet.
            tree(
                "3.0 (quilt)\n",
                &format!("{}{}{}", entry("1.0-3"), entry("1.0-2"), entry("11")),
            ),
        ];
        let fail = vec![
            tree(
                "3.0 (native)\n",
                &format!("{}{}", entry("1.0"), entry("0.1-2")),
            ),
            tree(
                "3.0 (quilt)\n",
                &format!("{}{}", entry("1:12-0.1"), entry("11")),
            ),
        ];
        SourceTreeTester::new(OddHistoricalDebianChangelogVersion, pass, fail).test_and_snapshot();
    }
}
