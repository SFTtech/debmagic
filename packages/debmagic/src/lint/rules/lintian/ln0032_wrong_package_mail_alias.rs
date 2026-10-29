use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::mailbox::parse_mailbox;

pub struct WrongPackageMailAlias;

declare_rule! {
    /// The Maintainer uses a `packages.debian.org` alias of another package.
    ///
    /// Reads the source paragraph of `debian/control`. The Maintainer must
    /// parse as one mailbox with a non-empty address whose host is exactly
    /// `packages.debian.org` and whose user differs from the `Source` name.
    /// Extra and Location are empty. Applies to the Source tree and Source
    /// package.
    WrongPackageMailAlias,
    code = "LN0032",
    tag = "wrong-package-mail-alias",
    default_selected = true,
    experimental = false,
}

impl SourceRule for WrongPackageMailAlias {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let source = control.source();
        let Some(maintainer) = source.get("Maintainer") else {
            return;
        };
        let Some(source_name) = source.get("Source") else {
            return;
        };
        let Some(mailbox) = parse_mailbox(maintainer) else {
            return;
        };
        if mailbox.host == "packages.debian.org" && mailbox.user != source_name.trim() {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(maintainer: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: {maintainer}\nStandards-Version: 4.7.2\n\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    type Files = Vec<Vec<(&'static str, TestFile)>>;

    fn cases() -> (Files, Files) {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            control("Jane <jane@example.org>"),
            control("Debian Example Team <example@packages.debian.org>"),
            control("example@packages.debian.org"),
            // Not a valid single mailbox.
            control("Team <other@packages.debian.org>, Jane <jane@example.org>"),
            control("Team <other@Packages.debian.org>"),
            control("Team <other@lists.debian.org>"),
        ];
        let fail = vec![
            control("Team <other@packages.debian.org>"),
            control("other@packages.debian.org"),
            control("Team <Example@packages.debian.org>"),
        ];
        (pass, fail)
    }

    #[test]
    fn test() {
        let (pass, fail) = cases();
        SourceTreeTester::new(WrongPackageMailAlias, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let (pass, fail) = cases();
        SourcePackageTester::new(
            WrongPackageMailAlias,
            pass.into_iter()
                .map(SourcePackageCase::from)
                .collect::<Vec<_>>(),
            fail.into_iter()
                .map(SourcePackageCase::from)
                .collect::<Vec<_>>(),
        )
        .test_and_snapshot();
    }
}
