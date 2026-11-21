use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::source::{changelog_location, rc_version_without_tilde};

pub struct RcVersionGreaterThanExpectedVersion;

declare_rule! {
    /// A first upload of a pre-release has a version that sorts after the
    /// release it precedes: `rc`, `alpha`, `beta`, `pre`, `preview` or
    /// `prerelease` without a `~` in front.
    ///
    /// For a non-native package the upstream version is examined, for a
    /// native one the version itself. The marker must follow a character
    /// other than `~` or a lower-case letter. Reported only when the Debian
    /// revision is `1`, `0`, `0.1` or `0ubuntu1` (or the package is native),
    /// and the version is not a source NMU. Extra is
    /// `<version> > <expected> (consider using <suggestion>)`, with the marker
    /// and a separator in front of it dropped for the expected version. The
    /// newest heading is the Location.
    RcVersionGreaterThanExpectedVersion,
    code = "LN1177",
    tag = "rc-version-greater-than-expected-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for RcVersionGreaterThanExpectedVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((line, extra)) = rc_version_without_tilde(ctx) else {
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

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            quilt("1.0-1", "unstable"),
            quilt("1.0~rc3-1", "unstable"),
            // Not the first Debian revision.
            quilt("1.0+rc3-2", "unstable"),
            // A source NMU.
            quilt("1.0+rc3-1.1", "unstable"),
            // A marker inside a word, or at the very start.
            quilt("1.0+arc3-1", "unstable"),
            quilt("rc3-1", "unstable"),
            quilt("1.0+ricercar-1", "unstable"),
            native("1.0~beta1", "unstable"),
        ];
        let fail = vec![
            quilt("1.0+rc3-1", "unstable"),
            quilt("1.0-rc3-1", "unstable"),
            quilt("1.0beta-1", "unstable"),
            quilt("1.0.alpha2-0ubuntu1", "unstable"),
            quilt("1.0+pre-0ubuntu1", "unstable"),
            quilt("1:2.0+preview1-1", "unstable"),
            quilt("2.0prerelease3-0", "unstable"),
            native("2.0+beta1", "unstable"),
            native("2.0.rc1", "unstable"),
        ];
        SourceTreeTester::new(RcVersionGreaterThanExpectedVersion, pass, fail).test_and_snapshot();
    }
}
