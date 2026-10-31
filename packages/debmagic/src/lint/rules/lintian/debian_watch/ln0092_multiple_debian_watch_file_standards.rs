use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::standard::{WatchHit, flag_watch};

pub struct MultipleDebianWatchFileStandards;

declare_rule! {
    /// `debian/watch` declares more than one standard.
    ///
    /// Extra is the standards in the order they appear, separated by spaces,
    /// including repeats. The path is the Location.
    MultipleDebianWatchFileStandards,
    code = "LN0092",
    tag = "multiple-debian-watch-file-standards",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MultipleDebianWatchFileStandards {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_watch(ctx, |hit| match hit {
            WatchHit::Multiple(versions) => Some(versions.clone()),
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
            tree("version=2\n\nversion=3\n"),
            tree("version=2\nversion=3\nversion=42\n"),
            tree("version=2\nversion=2\n"),
        ];
        SourceTreeTester::new(MultipleDebianWatchFileStandards, pass, fail).test_and_snapshot();
    }
}
