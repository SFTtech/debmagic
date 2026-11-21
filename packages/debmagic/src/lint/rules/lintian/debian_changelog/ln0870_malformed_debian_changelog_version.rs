use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::source::{changelog_location, package_is_native};
use super::version::parse_version;

pub struct MalformedDebianChangelogVersion;

declare_rule! {
    /// The newest changelog version does not parse for the package's nativeness.
    ///
    /// Parses the newest entry's version the way lintian does, once as native and
    /// once as non-native, and uses the nativeness the package has (see
    /// `package_is_native`). Fires when that parse fails. Extra is the version
    /// followed by `(for native)` or `(for non-native)`; the newest heading is the
    /// Location. A changelog without entries is skipped.
    MalformedDebianChangelogVersion,
    code = "LN0870",
    tag = "malformed-debian-changelog-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MalformedDebianChangelogVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let native = package_is_native(ctx);
        let Some(latest) = ctx
            .debian_changelog()
            .and_then(|changelog| changelog.entries().first())
            .map(|entry| (entry.version().to_string(), entry.line()))
        else {
            return;
        };
        let (version, line) = latest;
        if parse_version(&version, native).is_some() {
            return;
        }
        let kind = if native { "native" } else { "non-native" };
        ctx.diagnostic(
            Diagnostic::error(format!("{version} (for {kind})"))
                .with_location(changelog_location(line)),
        );
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

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("3.0 (quilt)\n", &entry("example", "1.0-1", NEW)),
            tree("3.0 (native)\n", &entry("example", "1.0", NEW)),
            tree("3.0 (native)\n", &entry("example", "2:1.0+nmu1", NEW)),
            tree("3.0 (quilt)\n", &entry("example", "2:1.0-1+deb9u1", NEW)),
        ];
        let fail = vec![
            tree("3.0 (native)\n", &entry("example", "1.0-1", NEW)),
            tree("3.0 (quilt)\n", &entry("example", "1.0", NEW)),
            tree("3.0 (quilt)\n", &entry("example", "1.0-", NEW)),
        ];
        SourceTreeTester::new(MalformedDebianChangelogVersion, pass, fail).test_and_snapshot();
    }
}
