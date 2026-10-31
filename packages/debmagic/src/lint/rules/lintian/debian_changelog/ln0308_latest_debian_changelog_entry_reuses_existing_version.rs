use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::source::{changelog_location, reused_version};

pub struct LatestDebianChangelogEntryReusesExistingVersion;

declare_rule! {
    /// The newest changelog version repeats an older one once the epoch is
    /// removed.
    ///
    /// The newest entry's parsed `no_epoch` is compared with each older
    /// version after a single leading epoch is stripped, and only when the
    /// source names match. The first hit wins. Extra is
    /// `latest ~ older (last used: date)`. The newest heading is the Location.
    LatestDebianChangelogEntryReusesExistingVersion,
    code = "LN0308",
    tag = "latest-debian-changelog-entry-reuses-existing-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for LatestDebianChangelogEntryReusesExistingVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((line, extra)) = reused_version(ctx) else {
            return;
        };
        ctx.diagnostic(Diagnostic::warn(extra).with_location(changelog_location(line)));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn entry(source: &str, version: &str, date: &str) -> String {
        format!(
            "{source} ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  {date}\n"
        )
    }

    fn tree(format: &str, changelog: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/source/format", TestFile::new(format)),
            ("debian/changelog", TestFile::new(changelog)),
        ]
    }

    const NEW: &str = "Tue, 30 Dec 2008 17:34:02 +0000";
    const OLD: &str = "Fri, 06 Feb 2008 22:22:37 -0800";

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("3.0 (quilt)\n", &entry("example", "1.0-1", NEW)),
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("example", "2.0-1", NEW),
                    entry("example", "1.0-1", OLD)
                ),
            ),
            // Same version text, different source.
            tree(
                "3.0 (native)\n",
                &format!(
                    "{}{}",
                    entry("example", "1.0", NEW),
                    entry("other", "1.0", OLD)
                ),
            ),
        ];
        let fail = vec![
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("example", "1:1.0-1", NEW),
                    entry("example", "1.0-1", OLD)
                ),
            ),
            tree(
                "3.0 (native)\n",
                &format!(
                    "{}{}{}",
                    entry("example", "3:1.0", NEW),
                    entry("other", "1.0", OLD),
                    entry("example", "2:1.0", OLD)
                ),
            ),
        ];
        SourceTreeTester::new(LatestDebianChangelogEntryReusesExistingVersion, pass, fail)
            .test_and_snapshot();
    }
}
