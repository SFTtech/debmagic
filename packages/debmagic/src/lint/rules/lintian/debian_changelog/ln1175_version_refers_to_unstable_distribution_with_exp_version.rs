use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::source::{changelog_location, exp_version_in_unstable};

pub struct VersionRefersToUnstableDistributionWithExpVersion;

declare_rule! {
    /// The newest changelog entry is for `unstable` but its version has `~exp`
    /// in the Debian revision, which is for `experimental`.
    ///
    /// Only the exact distribution `unstable` counts (`unstable experimental`
    /// does not). The revision is the part after the last hyphen of a
    /// non-native version, or the whole version of a native one, without a
    /// source NMU suffix. A version that does not parse for the package's
    /// nativeness is `malformed-debian-changelog-version` instead. Extra is
    /// the version; the newest heading is the Location.
    VersionRefersToUnstableDistributionWithExpVersion,
    code = "LN1175",
    tag = "version-refers-to-unstable-distribution-with-exp-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for VersionRefersToUnstableDistributionWithExpVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((line, extra)) = exp_version_in_unstable(ctx) else {
            return;
        };
        ctx.diagnostic(Diagnostic::warn(extra).with_location(changelog_location(line)));
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

    fn native(version: &str, distribution: &str) -> Vec<(&'static str, TestFile)> {
        tree(
            "3.0 (native)\n",
            &entry(version, distribution, "Change.", NEW),
        )
    }

    const NEW: &str = "Tue, 30 Dec 2008 17:34:02 +0000";
    const OLD: &str = "Fri, 06 Feb 2008 22:22:37 -0800";

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            quilt("1.0-1", "unstable"),
            quilt("1.0-1~exp1", "experimental"),
            quilt("1.0~exp1-1", "unstable"),
            quilt("1.0-1~exp1", "unstable experimental"),
            quilt("1.0-1~exp1", "UNRELEASED"),
        ];
        let fail = vec![
            quilt("1.0-1~exp1", "unstable"),
            quilt("2:1.0-1~exp2", "unstable"),
            native("1.0~exp1", "unstable"),
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("1.0-2~exp1", "unstable", "Change.", NEW),
                    entry("1.0-1", "unstable", "Change.", OLD)
                ),
            ),
        ];
        SourceTreeTester::new(
            VersionRefersToUnstableDistributionWithExpVersion,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
