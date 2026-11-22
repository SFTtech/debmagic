use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::{Severity, SourceRule};

use super::file::{WatchFileHit, flag_watch_file};

pub struct DebianWatchUsesInsecureUri;

declare_rule! {
    /// A watch line names an unencrypted `http:` or `ftp:` URI.
    ///
    /// Extra is the first such URI on the line, from its scheme to the next
    /// whitespace. A URI that starts with `//sf.net/` is not reported, as in
    /// lintian. The line is the Location. Reported at info severity.
    DebianWatchUsesInsecureUri,
    code = "LN1345",
    tag = "debian-watch-uses-insecure-uri",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DebianWatchUsesInsecureUri {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch_file(ctx, Severity::Info, |hit| {
            matches!(hit, WatchFileHit::InsecureUri(_))
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
            tree("version=4\nhttps://example.org/foo/ /(.+)\\.tar\\.gz\n"),
            tree("version=4\nhttp://sf.net/projects/foo/ /(.+)\\.tar\\.gz\n"),
        ];
        let fail = vec![tree(
            "version=4\nhttp://example.org/foo/ /(.+)\\.tar\\.gz\n",
        )];
        SourceTreeTester::new(DebianWatchUsesInsecureUri, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("version=4\n"))];
        let fail = vec![SourcePackageCase::from(tree(
            "version=4\nftp://example.org/foo/ /(.+)\\.tar\\.gz\n",
        ))];
        SourcePackageTester::new(DebianWatchUsesInsecureUri, pass, fail).test_and_snapshot();
    }
}
