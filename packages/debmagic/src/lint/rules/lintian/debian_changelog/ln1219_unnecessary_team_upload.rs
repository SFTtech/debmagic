use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::nmu::nmu_facts;
use super::source::changelog_location;

pub struct UnnecessaryTeamUpload;

declare_rule! {
    /// A team upload is signed by the Maintainer or an Uploader.
    ///
    /// The newest changelog entry's first bullet says `team upload`, but its
    /// trailer address is the Maintainer or one of the Uploaders, so it is a
    /// maintainer's upload and the wording is unnecessary. A QA upload
    /// (Maintainer `packages@qa.debian.org`) is someone else's Tag. Extra is
    /// empty. The newest heading is the Location.
    UnnecessaryTeamUpload,
    code = "LN1219",
    tag = "unnecessary-team-upload",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UnnecessaryTeamUpload {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = nmu_facts(ctx) else {
            return;
        };
        if !facts.unnecessary_team_upload {
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
            tree(&entry("1.0-1", "B Other <b@example.com>", "Team upload.")),
            tree(&entry(
                "1.0-1",
                "A Maintainer <a@example.com>",
                "Maintainer upload.",
            )),
        ];
        let fail = vec![
            tree(&entry(
                "1.0-1",
                "A Maintainer <a@example.com>",
                "Team upload.",
            )),
            tree(&entry(
                "1.0-1",
                "C Uploader <c@example.com>",
                "Team upload.",
            )),
        ];
        SourceTreeTester::new(UnnecessaryTeamUpload, pass, fail).test_and_snapshot();
    }
}
