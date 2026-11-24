use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::nmu::nmu_facts;
use super::source::changelog_location;

pub struct QaUploadHasIncorrectVersionNumber;

declare_rule! {
    /// A QA team's upload carries a single-NMU version number.
    ///
    /// Maintainer contains `packages@qa.debian.org` (domain compared
    /// case-insensitively). The version must not have exactly one NMU suffix:
    /// `-1.1` and `+nmu1` are the shapes that count. Extra is the version. The
    /// newest heading is the Location.
    QaUploadHasIncorrectVersionNumber,
    code = "LN0366",
    tag = "qa-upload-has-incorrect-version-number",
    default_selected = true,
    experimental = false,
}

impl SourceRule for QaUploadHasIncorrectVersionNumber {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = nmu_facts(ctx) else {
            return;
        };
        if !facts.qa_incorrect_version {
            return;
        }
        ctx.diagnostic(
            Diagnostic::warn(facts.version).with_location(changelog_location(facts.line)),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const QA: &str = "\
Source: example
Maintainer: Debian QA Group <packages@qa.debian.org>

Package: example
Architecture: all
Description: example
 extra
";

    const MAINTAINER: &str = "\
Source: example
Maintainer: A Maintainer <a@example.com>

Package: example
Architecture: all
Description: example
 extra
";

    fn entry(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * QA upload.\n\n -- Debian QA Group <packages@qa.debian.org>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(control: &str, version: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control)),
            ("debian/changelog", TestFile::new(entry(version))),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree(QA, "1.0-1"),
            tree(QA, "1.0-1.1.1"),
            tree(QA, "1.0-2"),
            // Not a QA upload: the NMU shape is judged elsewhere.
            tree(MAINTAINER, "1.0+nmu1"),
        ];
        let fail = vec![tree(QA, "1.0+nmu1"), tree(QA, "1.0-1.1")];
        SourceTreeTester::new(QaUploadHasIncorrectVersionNumber, pass, fail).test_and_snapshot();
    }
}
