use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::{Severity, SourceRule};

use super::file::{WatchFileHit, flag_watch_file};

pub struct DebianWatchFileSpecifiesWrongUpstreamVersion;

declare_rule! {
    /// A watch `uversion` is a changelog version, Debian revision included.
    ///
    /// Native packages are skipped. Extra is that version. The path is the
    /// Location.
    DebianWatchFileSpecifiesWrongUpstreamVersion,
    code = "LN0099",
    tag = "debian-watch-file-specifies-wrong-upstream-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianWatchFileSpecifiesWrongUpstreamVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch_file(ctx, Severity::Warning, |hit| {
            matches!(hit, WatchFileHit::WrongUpstreamVersion(_))
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn changelog(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * test\n\n -- Example <ex@example.com>  Tue, 31 Mar 2026 17:46:22 +0200\n"
        )
    }

    fn tree(version: &str, watch: &str) -> Vec<(String, TestFile)> {
        vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            (
                "debian/changelog".to_string(),
                TestFile::from(changelog(version)),
            ),
            ("debian/watch".to_string(), TestFile::from(watch)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![tree(
            "1.0-1",
            "version=4\nhttps://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz 1.0\n",
        )];
        let fail = vec![tree(
            "1.0-1",
            "version=4\nhttps://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz 1.0-1\n",
        )];
        SourceTreeTester::new(DebianWatchFileSpecifiesWrongUpstreamVersion, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "1.0-1",
            "version=4\nhttps://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "2.0.ds1-1",
            "version=4\nhttps://example.com/ foo-([\\d.]+)\\.tar\\.gz 2.0.ds1-1 uupdate\n",
        ))];
        SourcePackageTester::new(DebianWatchFileSpecifiesWrongUpstreamVersion, pass, fail)
            .test_and_snapshot();
    }
}
