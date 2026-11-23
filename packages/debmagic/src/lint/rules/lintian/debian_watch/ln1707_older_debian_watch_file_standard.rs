use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::{Severity, SourceRule};

use super::standard::{WatchHit, flag_watch_at};

pub struct OlderDebianWatchFileStandard;

declare_rule! {
    /// The highest known `debian/watch` standard is 4, which lintian considers
    /// newly superseded: upgrading to 5 is advised.
    ///
    /// Known standards are 2, 3, 4, and 5. Extra is the standard, `4`. The path
    /// is the Location. Reported at info severity. Standard 5 is current and
    /// below 4 is `obsolete-debian-watch-file-standard` (LN0090).
    OlderDebianWatchFileStandard,
    code = "LN1707",
    tag = "older-debian-watch-file-standard",
    default_selected = false,
    experimental = false,
}

impl SourceRule for OlderDebianWatchFileStandard {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch_at(ctx, Severity::Info, |hit| match hit {
            WatchHit::Older(version) => Some(version.clone()),
            _ => None,
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

    fn tree(watch: &str) -> Vec<(String, TestFile)> {
        vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            ("debian/watch".to_string(), TestFile::from(watch)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("version=5\nhttp://example.com\n"),
            tree("version=4\nhttp://example.com\n\nversion=5\nhttp://example.com\n"),
            tree("version=3\nhttp://example.com\n"),
            tree("version=1\nhttp://example.com\n"),
            tree("http://example.com\n"),
        ];
        let fail = vec![tree("version=4\nhttp://example.com\n")];
        SourceTreeTester::new(OlderDebianWatchFileStandard, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/watch".to_string(),
            TestFile::from("version=5\n"),
        )])];
        let fail = vec![SourcePackageCase::from(vec![(
            "debian/watch".to_string(),
            TestFile::from("version=4\n"),
        )])];
        SourcePackageTester::new(OlderDebianWatchFileStandard, pass, fail).test_and_snapshot();
    }
}
