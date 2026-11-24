use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::source::{changelog_location, new_upstream_without_new_version};

pub struct PossibleNewUpstreamReleaseWithoutNewVersion;

declare_rule! {
    /// The newest changelog entry says it is a new upstream release but has the
    /// same upstream version as the entry before it.
    ///
    /// Needs two entries whose versions parse, with the same source name,
    /// epoch and upstream version. The newest entry's changes must have a line
    /// that starts with `* New upstream [<word>] release` (any letter case,
    /// any whitespace). Never for a native package. Extra is empty; the
    /// newest heading is the Location.
    PossibleNewUpstreamReleaseWithoutNewVersion,
    code = "LN1178",
    tag = "possible-new-upstream-release-without-new-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PossibleNewUpstreamReleaseWithoutNewVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(line) = new_upstream_without_new_version(ctx) else {
            return;
        };
        ctx.diagnostic(Diagnostic::warn("").with_location(changelog_location(line)));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn entry(version: &str, distribution: &str, changes: &str, date: &str) -> String {
        format!(
            "example ({version}) {distribution}; urgency=low\n\n  * {changes}\n\n -- A Uthor <a@example.com>  {date}\n"
        )
    }

    fn tree(format: &str, changelog: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/source/format", TestFile::new(format)),
            ("debian/changelog", TestFile::new(changelog)),
        ]
    }

    /// A non-native package whose newest entry is `version` for `distribution`.
    fn quilt(version: &str, distribution: &str) -> Vec<(&'static str, TestFile)> {
        tree(
            "3.0 (quilt)\n",
            &entry(version, distribution, "Change.", NEW),
        )
    }

    const NEW: &str = "Tue, 30 Dec 2008 17:34:02 +0000";
    const OLD: &str = "Fri, 06 Feb 2008 22:22:37 -0800";

    #[test]
    fn test() {
        let release = |version: &str, changes: &str| {
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry(version, "unstable", changes, NEW),
                    entry("1.0-1", "unstable", "Initial release.", OLD)
                ),
            )
        };
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            quilt("1.0-1", "unstable"),
            // A new upstream version.
            release("1.1-1", "New upstream release."),
            // Not announced as a new upstream release.
            release("1.0-2", "Fix a bug."),
            release("1.0-2", "Not a new upstream release."),
            // A different epoch.
            release("1:1.0-2", "New upstream release."),
            // Native packages never.
            tree(
                "3.0 (native)\n",
                &format!(
                    "{}{}",
                    entry("1.0+1", "unstable", "New upstream release.", NEW),
                    entry("1.0", "unstable", "Initial release.", OLD)
                ),
            ),
            // A different source.
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("1.0-2", "unstable", "New upstream release.", NEW),
                    "other (1.0-1) unstable; urgency=low\n\n  * x\n\n -- A <a@example.com>  Fri, 06 Feb 2008 22:22:37 -0800\n"
                ),
            ),
        ];
        let fail = vec![
            release("1.0-2", "New upstream release."),
            release("1.0-2", "new  upstream\tversion release"),
            release("1.0-2", "NEW UPSTREAM RELEASE"),
            release("1.0-1.1", "New upstream release."),
        ];
        SourceTreeTester::new(PossibleNewUpstreamReleaseWithoutNewVersion, pass, fail)
            .test_and_snapshot();
    }
}
