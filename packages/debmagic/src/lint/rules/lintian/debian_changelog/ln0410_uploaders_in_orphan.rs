use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::nmu::uploaders_in_orphan_line;
use super::source::changelog_location;

pub struct UploadersInOrphan;

declare_rule! {
    /// An orphaned package still declares `Uploaders`.
    ///
    /// Maintainer contains `packages@qa.debian.org` after the domain is
    /// lowercased, and the source paragraph declares `Uploaders`. The newest
    /// changelog entry must parse; the Check returns before Maintainer when
    /// it does not. Extra is empty. The Location is that changelog heading.
    /// Applies to the Source tree and Source package.
    UploadersInOrphan,
    code = "LN0410",
    tag = "uploaders-in-orphan",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UploadersInOrphan {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(line) = uploaders_in_orphan_line(ctx) else {
            return;
        };
        ctx.diagnostic(Diagnostic::error("").with_location(changelog_location(line)));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(maintainer: &str, uploaders: bool) -> Vec<(&'static str, TestFile)> {
        let uploaders = if uploaders {
            "Uploaders: Niels Thykier <niels@thykier.net>\n"
        } else {
            ""
        };
        vec![
            (
                "debian/control",
                TestFile::new(format!(
                    "Source: example\nMaintainer: {maintainer}\n{uploaders}\n\
                     Package: example\nArchitecture: all\nDescription: example\n extra\n"
                )),
            ),
            (
                "debian/changelog",
                TestFile::new(
                    "example (1.0-1) unstable; urgency=low\n\n  * QA upload.\n\n -- Niels Thykier <niels@thykier.net>  Tue, 30 Dec 2008 17:34:02 +0000\n",
                ),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("Debian QA Group <packages@qa.debian.org>", false),
            tree("A Maintainer <a@example.com>", true),
            vec![(
                "debian/control",
                TestFile::new(
                    "Source: example\nMaintainer: Debian QA Group <packages@qa.debian.org>\n\
                     Uploaders: Niels Thykier <niels@thykier.net>\n\n\
                     Package: example\nArchitecture: all\nDescription: example\n extra\n",
                ),
            )],
        ];
        let fail = vec![
            tree("Debian QA Group <packages@qa.debian.org>", true),
            tree("QA group <packages@QA.debian.org>", true),
        ];
        SourceTreeTester::new(UploadersInOrphan, pass, fail).test_and_snapshot();
    }
}
