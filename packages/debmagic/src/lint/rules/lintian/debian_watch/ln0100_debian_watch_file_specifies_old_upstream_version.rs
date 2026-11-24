use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::{Severity, SourceRule};

use super::file::{WatchFileHit, flag_watch_file};

pub struct DebianWatchFileSpecifiesOldUpstreamVersion;

declare_rule! {
    /// A watch `uversion` matches an older changelog upstream version.
    ///
    /// The Debian revision and epoch are stripped before the compare. The
    /// newest entry is not old. Extra is that version. The path is the
    /// Location.
    DebianWatchFileSpecifiesOldUpstreamVersion,
    code = "LN0100",
    tag = "debian-watch-file-specifies-old-upstream-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianWatchFileSpecifiesOldUpstreamVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch_file(ctx, Severity::Warning, |hit| {
            matches!(hit, WatchFileHit::OldUpstreamVersion(_))
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

    fn changelog() -> String {
        "example (2.0-1) unstable; urgency=low\n\n  * test\n\n -- Example <ex@example.com>  Tue, 31 Mar 2026 17:46:22 +0200\n\nexample (1.0-1) unstable; urgency=low\n\n  * test\n\n -- Example <ex@example.com>  Thu, 01 Jan 2009 08:34:20 -0800\n"
            .to_string()
    }

    fn tree(watch: &str) -> Vec<(String, TestFile)> {
        vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            ("debian/changelog".to_string(), TestFile::from(changelog())),
            ("debian/watch".to_string(), TestFile::from(watch)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![tree(
            "version=4\nhttps://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz 2.0\n",
        )];
        let fail = vec![tree(
            "version=4\nhttps://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz 1.0\n",
        )];
        SourceTreeTester::new(DebianWatchFileSpecifiesOldUpstreamVersion, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "version=4\nhttps://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz 2.0\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "version=4\nhttps://example.com/dist/ foo\\.([\\d.]+)\\.tar\\.gz 1.0\n",
        ))];
        SourcePackageTester::new(DebianWatchFileSpecifiesOldUpstreamVersion, pass, fail)
            .test_and_snapshot();
    }
}
