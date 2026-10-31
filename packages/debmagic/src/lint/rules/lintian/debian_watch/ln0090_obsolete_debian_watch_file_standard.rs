use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::standard::{WatchHit, flag_watch};

pub struct ObsoleteDebianWatchFileStandard;

declare_rule! {
    /// The highest known `debian/watch` standard is older than 4.
    ///
    /// Known standards are 2, 3, 4, and 5. Extra is that highest standard.
    /// The path is the Location. Standards 4 and 5 are not obsolete.
    ObsoleteDebianWatchFileStandard,
    code = "LN0090",
    tag = "obsolete-debian-watch-file-standard",
    default_selected = true,
    experimental = false,
}

impl SourceRule for ObsoleteDebianWatchFileStandard {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch(ctx, |hit| match hit {
            WatchHit::Obsolete(version) => Some(version.clone()),
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
            tree("version=4\nhttp://example.com\n"),
            tree("version=5\nhttp://example.com\n"),
            tree("version=1\nhttp://example.com\n"),
            tree("http://example.com\n"),
        ];
        let fail = vec![
            tree("version=2\nhttp://example.com\n"),
            tree("version=3\nhttp://example.com\n"),
            tree("version=2\nhttp://example.com\n\nversion=3\nhttp://example.com\n"),
        ];
        SourceTreeTester::new(ObsoleteDebianWatchFileStandard, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/watch".to_string(),
            TestFile::from("version=4\n"),
        )])];
        let fail = vec![SourcePackageCase::from(vec![(
            "debian/watch".to_string(),
            TestFile::from("version=3\n"),
        )])];
        SourcePackageTester::new(ObsoleteDebianWatchFileStandard, pass, fail).test_and_snapshot();
    }
}
