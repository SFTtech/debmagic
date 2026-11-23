use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::source::{changelog_location, latest};

pub struct HyphenInUpstreamPartOfDebianChangelogVersion;

declare_rule! {
    /// The upstream part of the newest changelog version of a non-native
    /// package contains a hyphen.
    ///
    /// Policy allows it, but some tools may croak. The version must parse for
    /// the package's nativeness, else the Tag is quiet (lintian reports
    /// `malformed-debian-changelog-version` instead). A native package has no
    /// upstream part to judge. Extra is the upstream version, epoch and Debian
    /// revision removed, for example `1.5+important~personal1-stable`. The
    /// Location is the newest entry's heading. Severity is pedantic and the Tag
    /// is not selected by default, as in lintian.
    HyphenInUpstreamPartOfDebianChangelogVersion,
    code = "LN1661",
    tag = "hyphen-in-upstream-part-of-debian-changelog-version",
    default_selected = false,
    experimental = false,
}

impl SourceRule for HyphenInUpstreamPartOfDebianChangelogVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(latest) = latest(ctx) else {
            return;
        };
        if latest.native || !latest.version.upstream.contains('-') {
            return;
        }
        ctx.diagnostic(
            Diagnostic::warn(latest.version.upstream.clone())
                .with_severity(Severity::Pedantic)
                .with_location(changelog_location(latest.line)),
        );
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
            tree("3.0 (native)\n", &entry("1.0-1")),
            tree("3.0 (native)\n", &entry("1.0")),
            // Only the newest entry is judged.
            tree(
                "3.0 (quilt)\n",
                &format!("{}{}", entry("1.0-1"), entry("1.0-2-1")),
            ),
            // The version does not parse, so there is no upstream part.
            tree("3.0 (quilt)\n", &entry("1.0-1-")),
        ];
        let fail = vec![
            tree("3.0 (quilt)\n", &entry("4.6.2-1-1")),
            tree(
                "3.0 (quilt)\n",
                &entry("2:1.5+important~personal1-stable-7"),
            ),
        ];
        SourceTreeTester::new(HyphenInUpstreamPartOfDebianChangelogVersion, pass, fail)
            .test_and_snapshot();
    }
}
