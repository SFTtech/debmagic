use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::mailbox::parse_mailbox;

pub struct NoHumanMaintainers;

/// Hosts whose addresses are lists or aliases, not people
/// (`%REQ_HUMAN_UPLOADER_HOSTS` in lintian's `fields/maintainer` Check).
const NON_HUMAN_HOSTS: &[&str] = &[
    "lists.debian.org",
    "lists.alioth.debian.org",
    "tracker.debian.org",
    "security.debian.org",
    "packages.debian.org",
];

declare_rule! {
    /// The Maintainer is a mailing list or alias and no Uploaders are declared.
    ///
    /// Reads the source paragraph of `debian/control`. The Maintainer must
    /// parse as one mailbox with a non-empty address whose host (case
    /// sensitive, as lintian) is `lists.debian.org`, `lists.alioth.debian.org`,
    /// `tracker.debian.org`, `security.debian.org`, or `packages.debian.org`.
    /// An `Uploaders` field of any value, even empty, silences the Tag.
    /// Extra and Location are empty. Applies to the Source
    /// tree and Source package.
    NoHumanMaintainers,
    code = "LN0021",
    tag = "no-human-maintainers",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NoHumanMaintainers {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let source = control.source();
        let Some(maintainer) = source.get("Maintainer") else {
            return;
        };
        let Some(mailbox) = parse_mailbox(maintainer) else {
            return;
        };
        if NON_HUMAN_HOSTS.contains(&mailbox.host) && !source.declares("Uploaders") {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\n{fields}Standards-Version: 4.7.2\n\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            control(""),
            control("Maintainer: Jane <jane@example.com>\n"),
            control(
                "Maintainer: Team <team@lists.debian.org>\nUploaders: Jane <jane@example.com>\n",
            ),
            control("Maintainer: Team <team@lists.debian.org>\nUploaders:\n"),
            control("Maintainer: Team <team@Lists.Debian.org>\n"),
            control("Maintainer: Team <team@lists.debian.org.example.com>\n"),
            control("Maintainer: not an address\n"),
            control("Maintainer: <@lists.debian.org>\n"),
        ];
        let fail = vec![
            control("Maintainer: Team <team@lists.debian.org>\n"),
            control("Maintainer: Team <team@lists.alioth.debian.org>\n"),
            control("Maintainer: Team <team@tracker.debian.org>\n"),
            control("Maintainer: Team <team@security.debian.org>\n"),
            control("Maintainer: Team <wrong@packages.debian.org>\n"),
            control("Maintainer: team@lists.debian.org\n"),
            control("Maintainer: \"Team, The\" <team@lists.debian.org>\n"),
        ];
        SourceTreeTester::new(NoHumanMaintainers, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(control(
            "Maintainer: Jane <jane@example.com>\n",
        ))];
        let fail = vec![SourcePackageCase::from(control(
            "Maintainer: Team <team@lists.debian.org>\n",
        ))];
        SourcePackageTester::new(NoHumanMaintainers, pass, fail).test_and_snapshot();
    }
}
