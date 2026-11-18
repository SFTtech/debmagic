use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::nmu::nmu_facts;
use super::source::changelog_location;

pub struct ExtraWhitespaceAroundNameInChangelogTrailer;

declare_rule! {
    /// The newest entry's trailer name has leading or trailing whitespace.
    ///
    /// The format is `-- NAME <EMAIL>  DATE`, with exactly one space after
    /// `--`. The trailer address, after the domain is lowercased, is compared
    /// with its surrounding whitespace removed. QA and team uploads are checked
    /// too. Extra is empty. The newest heading is the Location.
    ExtraWhitespaceAroundNameInChangelogTrailer,
    code = "LN0756",
    tag = "extra-whitespace-around-name-in-changelog-trailer",
    default_selected = true,
    experimental = false,
}

impl SourceRule for ExtraWhitespaceAroundNameInChangelogTrailer {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = nmu_facts(ctx) else {
            return;
        };
        if !facts.trailer_padded {
            return;
        }
        ctx.diagnostic(Diagnostic::warn("").with_location(changelog_location(facts.line)));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const MAINTAINER: &str = "\
Source: example
Maintainer: A Maintainer <a@example.com>

Package: example
Architecture: all
Description: example
 extra
";

    fn entry(trailer: &str) -> String {
        format!(
            "example (1.0-1) unstable; urgency=low\n\n  * Change.\n\n -- {trailer}  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(trailer: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(MAINTAINER)),
            ("debian/changelog", TestFile::new(entry(trailer))),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("A Maintainer <a@example.com>"),
            tree("B Other <b@example.com>"),
        ];
        let fail = vec![
            tree(" A Maintainer <a@example.com>"),
            tree("  A Maintainer <a@example.com>"),
            tree(" B Other <b@example.com>"),
        ];
        SourceTreeTester::new(ExtraWhitespaceAroundNameInChangelogTrailer, pass, fail)
            .test_and_snapshot();
    }
}
