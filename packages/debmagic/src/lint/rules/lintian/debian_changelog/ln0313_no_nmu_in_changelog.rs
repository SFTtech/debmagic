use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::nmu::nmu_facts;
use super::source::changelog_location;

pub struct NoNmuInChangelog;

declare_rule! {
    /// An NMU changelog's first bullet does not say that it is an NMU.
    ///
    /// The trailer address, after the domain is lowercased and surrounding
    /// space is removed, is neither Maintainer nor an Uploader. QA and team
    /// uploads, a `local` version, and a first bullet that says `local
    /// package` are someone else's Tags. An acknowledgement of an NMU does
    /// not count as announcing one. Extra is empty. The newest heading is
    /// the Location.
    NoNmuInChangelog,
    code = "LN0313",
    tag = "no-nmu-in-changelog",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NoNmuInChangelog {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = nmu_facts(ctx) else {
            return;
        };
        if !facts.no_nmu {
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
            tree(&entry(
                "1.0-1",
                "A Maintainer <a@example.com>",
                "Maintainer upload.",
            )),
            tree(&entry(
                "1.0-1",
                "C Uploader <c@example.com>",
                "Uploader upload.",
            )),
            tree(&entry(
                "1.0-1.1",
                "B Other <b@example.com>",
                "NMU to fix the build.",
            )),
            tree(&entry(
                "1.0-1",
                "B Other <b@example.com>",
                "This is a local package.",
            )),
            tree(&entry("1.0+local1", "B Other <b@example.com>", "Rebuild.")),
            tree(&entry("1.0-1", "B Other <b@example.com>", "Team upload.")),
        ];
        let fail = vec![
            tree(&entry("1.0", "B Other <b@example.com>", "Rebuild.")),
            tree(&entry("1.0-1.1", "B Other <b@example.com>", "Fix a typo.")),
            tree(&entry(
                "1.0-1",
                "B Other <B@Example.COM>",
                "Domain case differs.",
            )),
        ];
        SourceTreeTester::new(NoNmuInChangelog, pass, fail).test_and_snapshot();
    }
}
