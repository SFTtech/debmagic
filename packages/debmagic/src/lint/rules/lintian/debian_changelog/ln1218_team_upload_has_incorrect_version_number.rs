use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::nmu::nmu_facts;
use super::source::changelog_location;

pub struct TeamUploadHasIncorrectVersionNumber;

declare_rule! {
    /// A team upload has an NMU revision number.
    ///
    /// The newest changelog entry's first bullet says `team upload`, and the
    /// Version has an NMU suffix (`-1.1`, `+nmuN`, or `+bN`). A QA upload
    /// (Maintainer `packages@qa.debian.org`) is someone else's Tag and takes
    /// precedence over the team wording. Extra is the version. The newest
    /// heading is the Location.
    TeamUploadHasIncorrectVersionNumber,
    code = "LN1218",
    tag = "team-upload-has-incorrect-version-number",
    default_selected = true,
    experimental = false,
}

impl SourceRule for TeamUploadHasIncorrectVersionNumber {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = nmu_facts(ctx) else {
            return;
        };
        if !facts.team_incorrect_version {
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

    const MAINTAINER: &str = "\
Source: example
Maintainer: A Maintainer <a@example.com>
Uploaders: C Uploader <c@example.com>

Package: example
Architecture: all
Description: example
 extra
";

    fn entry(version: &str, trailer: &str, body: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * {body}\n\n -- {trailer}  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(changelog: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(MAINTAINER)),
            ("debian/changelog", TestFile::new(changelog)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree(&entry(
                "1.0-1",
                "A Maintainer <a@example.com>",
                "Team upload.",
            )),
            tree(&entry(
                "1.0+nmu1",
                "A Maintainer <a@example.com>",
                "Maintainer upload.",
            )),
            tree(&entry(
                "1.0-1.1",
                "A Maintainer <a@example.com>",
                "Maintainer upload.",
            )),
        ];
        let fail = vec![
            tree(&entry(
                "1.0+nmu1",
                "A Maintainer <a@example.com>",
                "Team upload.",
            )),
            tree(&entry("1.0-1.1", "B Other <b@example.com>", "Team upload.")),
            tree(&entry(
                "1.0-1.1",
                "C Uploader <c@example.com>",
                "Team upload.",
            )),
        ];
        SourceTreeTester::new(TeamUploadHasIncorrectVersionNumber, pass, fail).test_and_snapshot();
    }
}
