use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::nmu::nmu_facts;
use super::source::changelog_location;

pub struct MaintainerUploadHasIncorrectVersionNumber;

declare_rule! {
    /// A maintainer's upload has a Debian revision with dots.
    ///
    /// The trailer address is the Maintainer or an Uploader, so the upload is
    /// not an NMU, yet the version has an NMU suffix: `-1.1`, `+nmuN`, or
    /// `+bN`. QA and team uploads are someone else's Tags. A `local` package
    /// or `local` version does not exempt the entry. Extra is the version.
    /// The newest heading is the Location.
    MaintainerUploadHasIncorrectVersionNumber,
    code = "LN0754",
    tag = "maintainer-upload-has-incorrect-version-number",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MaintainerUploadHasIncorrectVersionNumber {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = nmu_facts(ctx) else {
            return;
        };
        if !facts.maintainer_incorrect_version {
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
                "Maintainer upload.",
            )),
            tree(&entry(
                "1.0+local1",
                "A Maintainer <a@example.com>",
                "Local rebuild.",
            )),
            tree(&entry("1.0-1.1", "B Other <b@example.com>", "NMU.")),
            tree(&entry(
                "1.0-1.1",
                "A Maintainer <a@example.com>",
                "Team upload.",
            )),
        ];
        let fail = vec![
            tree(&entry(
                "1.0-1.1",
                "C Uploader <c@example.com>",
                "Uploader upload.",
            )),
            tree(&entry(
                "1.0-1.1",
                "A Maintainer <a@example.com>",
                "Maintainer upload.",
            )),
            tree(&entry(
                "1.0+nmu1",
                "A Maintainer <a@example.com>",
                "Maintainer upload.",
            )),
            tree(&entry(
                "1.0-1+b1",
                "A Maintainer <a@example.com>",
                "Maintainer upload.",
            )),
            tree(&entry(
                "1.0-1.1",
                "A Maintainer <a@example.com>",
                "This is a local package.",
            )),
        ];
        SourceTreeTester::new(MaintainerUploadHasIncorrectVersionNumber, pass, fail)
            .test_and_snapshot();
    }
}
