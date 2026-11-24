use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::nmu::nmu_facts;
use super::source::changelog_location;

pub struct NoQaInChangelog;

declare_rule! {
    /// A QA team's upload does not say so in the first changelog bullet.
    ///
    /// Maintainer contains `packages@qa.debian.org` (domain compared
    /// case-insensitively). The first `*` line must mention `orphan` or
    /// `QA upload` / `QA group upload`. Extra is empty. The newest heading
    /// is the Location.
    NoQaInChangelog,
    code = "LN0315",
    tag = "no-qa-in-changelog",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NoQaInChangelog {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = nmu_facts(ctx) else {
            return;
        };
        if !facts.no_qa {
            return;
        }
        ctx.diagnostic(Diagnostic::warn("").with_location(changelog_location(facts.line)));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const QA: &str = "\
Source: example
Maintainer: Debian QA Group <packages@qa.debian.org>
Uploaders: Niels Thykier <niels@thykier.net>

Package: example
Architecture: all
Description: example
 extra
";

    fn entry(body: &str) -> String {
        format!(
            "example (1.0+nmu1) unstable; urgency=low\n\n  * {body}\n\n -- Niels Thykier <niels@thykier.net>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(control: &str, changelog: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control)),
            ("debian/changelog", TestFile::new(changelog)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree(QA, &entry("QA upload.")),
            tree(QA, &entry("QA group upload to fix the RC bug.")),
            tree(QA, &entry("Orphan the package.")),
            tree(
                "\
Source: example
Maintainer: A Maintainer <a@example.com>

Package: example
Architecture: all
Description: example
 extra
",
                &entry("Ordinary upload with no QA note."),
            ),
        ];
        let fail = vec![
            tree(QA, &entry("Lintian Test Suite.")),
            tree(
                "\
Source: example
Maintainer: QA group <packages@QA.debian.org>

Package: example
Architecture: all
Description: example
 extra
",
                &entry("Doing an upload for QA but forgetting to say so."),
            ),
        ];
        SourceTreeTester::new(NoQaInChangelog, pass, fail).test_and_snapshot();
    }
}
