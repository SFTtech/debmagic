use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::source::{changelog_location, version_refers_to_distribution};

pub struct VersionRefersToDistribution;

declare_rule! {
    /// The Debian revision of the newest version names a suite or a release.
    ///
    /// Reported when the revision has `testing`, `stable` or `unstable` in any
    /// letter case, or when the entry is for `unstable` or `experimental` and
    /// the revision has the name of a Debian release series (lintian takes the
    /// series from `distro-info`; the vendored list is `sid` and `rc-buggy`
    /// excluded, `buzz` to the current development release). A source NMU
    /// (`1.0-1.1`, `1.0+nmu1`) is exempt. Series names are short, so `bo`,
    /// `rex` or `etch` anywhere in the revision count, as in lintian. Extra
    /// is the version; the newest heading is the Location.
    VersionRefersToDistribution,
    code = "LN1176",
    tag = "version-refers-to-distribution",
    default_selected = true,
    experimental = false,
}

impl SourceRule for VersionRefersToDistribution {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((line, extra)) = version_refers_to_distribution(ctx) else {
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
            // The upstream version is not the revision.
            quilt("1.0etch-1", "unstable"),
            quilt("1.0-1", "experimental"),
            // A release series only counts for `unstable` and `experimental`.
            quilt("1.0-1etch1", "stable"),
            quilt("1.0-1~bpo10+1", "unstable"),
            // A source NMU gets a free pass.
            quilt("1.0-1.1", "unstable"),
            quilt("1.0-1etch1.1", "unstable"),
            native("1.0+nmu1", "unstable"),
        ];
        let fail = vec![
            quilt("1.0-1etch1", "unstable"),
            quilt("1.0-1etch1", "experimental"),
            quilt("1.0-1bookworm", "unstable"),
            quilt("1.0-1stable1", "UNRELEASED"),
            quilt("1.0-1~Testing1", "UNRELEASED"),
            quilt("1.0-1unstable", "unstable"),
            quilt("3:1.0-1sarge1", "unstable"),
            native("1.0stable", "unstable"),
        ];
        SourceTreeTester::new(VersionRefersToDistribution, pass, fail).test_and_snapshot();
    }
}
