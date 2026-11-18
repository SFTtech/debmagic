use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::nmu::nmu_facts;
use super::source::changelog_location;

pub struct NmuInChangelog;

declare_rule! {
    /// A maintainer's upload says that it is an NMU.
    ///
    /// The first bullet mentions NMU or non-maintainer upload, and is not an
    /// acknowledgement of one, but the trailer address is the Maintainer or
    /// an Uploader. QA and team uploads are someone else's Tags. A `local
    /// package` or `local` version does not exempt the entry. Extra is empty.
    /// The newest heading is the Location.
    NmuInChangelog,
    code = "LN0753",
    tag = "nmu-in-changelog",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NmuInChangelog {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = nmu_facts(ctx) else {
            return;
        };
        if !facts.nmu_in_changelog {
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
                "1.0-1.1",
                "B Other <b@example.com>",
                "NMU to fix the build.",
            )),
            tree(&entry(
                "1.0-1",
                "A Maintainer <a@example.com>",
                "Acknowledge NMU.",
            )),
            tree(&entry(
                "1.0-1",
                "C Uploader <c@example.com>",
                "Uploader upload.",
            )),
            tree(&entry(
                "1.0-1",
                "A Maintainer <a@example.com>",
                "Team upload, NMU wording.",
            )),
        ];
        let fail = vec![
            tree(&entry("1.0-1", "A Maintainer <a@example.com>", "NMU.")),
            tree(&entry(
                "1.0-1",
                "A Maintainer <a@example.com>",
                "This is a local package. Not an NMU.",
            )),
            tree(&entry(
                "1.0-1",
                "A Maintainer <a@example.com>",
                "Non-maintainer upload.",
            )),
            tree(&entry(
                "1.0-1",
                "A Maintainer <a@Example.COM>",
                "Domain case differs, NMU.",
            )),
        ];
        SourceTreeTester::new(NmuInChangelog, pass, fail).test_and_snapshot();
    }
}
