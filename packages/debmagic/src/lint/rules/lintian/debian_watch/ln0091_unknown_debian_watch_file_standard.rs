use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::standard::{WatchHit, flag_watch};

pub struct UnknownDebianWatchFileStandard;

declare_rule! {
    /// `debian/watch` declares a standard other than 2, 3, 4, or 5.
    ///
    /// Each distinct unknown standard is reported, in the order it appears.
    /// Extra is that standard. The path is the Location.
    UnknownDebianWatchFileStandard,
    code = "LN0091",
    tag = "unknown-debian-watch-file-standard",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UnknownDebianWatchFileStandard {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch(ctx, |hit| match hit {
            WatchHit::Unknown(version) => Some(version.clone()),
            _ => None,
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

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
            tree("version=2\n"),
            tree("http://example.com\n"),
        ];
        let fail = vec![
            tree("version=1\n"),
            tree("version=0\n"),
            tree("version=6\n"),
            tree("version=42\n"),
            tree("Version: 7\n"),
            tree("version=1\nversion=6\nversion=1\n"),
        ];
        SourceTreeTester::new(UnknownDebianWatchFileStandard, pass, fail).test_and_snapshot();
    }
}
