use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::super::team_pkg_perl::{is_pkg_perl, source_fields, vcs_field};
use super::VCS_FIELDS;

pub struct NoTeamUrl;

declare_rule! {
    /// A pkg-perl package (its Maintainer names `pkg-perl-maintainers@lists.alioth.debian.org`)
    /// has a `Vcs-*` field whose value does not start with
    /// `https://salsa.debian.org/perl-team/modules/packages`, the team's
    /// repository location (matched case-insensitively). Covers the non-git
    /// fields as well as `Vcs-Git` and `Vcs-Browser`. One warning per field, with
    /// the field's name as written and its value as the extra.
    ///
    /// Lintian's `team/pkg-perl/vcs` Check. Reads the Dsc, or the source
    /// paragraph of `debian/control` on a Source tree; on a Source tree an `XS-`
    /// field counts under its plain name, and it is named that way in the extra.
    /// Location is empty.
    NoTeamUrl,
    code = "LN1785",
    tag = "no-team-url",
    default_selected = true,
    experimental = false,
}

/// Lintian's `^https://salsa\.debian\.org/perl-team/modules/packages`, with `/i`.
const TEAM_URL: &str = "https://salsa.debian.org/perl-team/modules/packages";

impl SourceRule for NoTeamUrl {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let source_tree = ctx.dsc().is_none();
        let outside: Vec<(String, String)> = {
            let Some(fields) = source_fields(ctx) else {
                return;
            };
            if !is_pkg_perl(fields) {
                return;
            }
            VCS_FIELDS
                .into_iter()
                .filter_map(|name| vcs_field(fields, name, source_tree))
                .filter(|(_, value)| !is_team_url(value))
                .collect()
        };
        for (name, value) in outside {
            ctx.diagnostic(Diagnostic::warn(format!("{name} {value}")));
        }
    }
}

fn is_team_url(value: &str) -> bool {
    value
        .get(..TEAM_URL.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(TEAM_URL))
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const PKG_PERL: &str = "Debian Perl Group <pkg-perl-maintainers@lists.alioth.debian.org>";
    const TEAM_GIT: &str = "https://salsa.debian.org/perl-team/modules/packages/libfoo-perl.git";

    fn control(maintainer: &str, fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: libfoo-perl\nMaintainer: {maintainer}\n{fields}\
                 Standards-Version: 4.7.2\nSection: perl\n\
                 \nPackage: libfoo-perl\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    fn dsc(maintainer: &str, fields: &str) -> Dsc {
        Dsc::parse(&format!(
            "Source: libfoo-perl\nMaintainer: {maintainer}\n{fields}"
        ))
        .expect("Dsc fixture")
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            control(
                PKG_PERL,
                &format!("Vcs-Git: {TEAM_GIT}\nVcs-Browser: {TEAM_GIT}\n"),
            ),
            // The team location is matched case-insensitively.
            control(
                PKG_PERL,
                "Vcs-Git: HTTPS://Salsa.Debian.org/perl-team/modules/packages/x.git\n",
            ),
            // Not a pkg-perl package.
            control(
                "Foo <foo@example.com>",
                "Vcs-Git: https://example.com/x.git\n",
            ),
            Vec::<(&str, TestFile)>::new(),
            // A non-UTF-8 debian/control is not read at all.
            vec![(
                "debian/control",
                TestFile::from_bytes(
                    b"Source: libfoo-perl\nMaintainer: \xff\nVcs-Git: x\n".to_vec(),
                ),
            )],
        ];
        let fail = vec![
            control(
                PKG_PERL,
                "Vcs-Git: https://salsa.debian.org/foo/libfoo-perl.git\n",
            ),
            control(
                PKG_PERL,
                "Vcs-Browser: https://salsa.debian.org/foo/libfoo-perl\n",
            ),
            // A non-git field is checked for the team location as well.
            control(PKG_PERL, "Vcs-Svn: svn://example.com/trunk\n"),
            // The prefix must match in full: a longer lookalike host is outside.
            control(
                PKG_PERL,
                "Vcs-Git: https://salsa.debian.org.evil.example/perl-team/modules/packages/x.git\n",
            ),
            // An empty value is outside the team location.
            control(PKG_PERL, "Vcs-Git:\n"),
            // Several fields: one warning each, in the Check's order.
            control(
                PKG_PERL,
                "Vcs-Browser: https://example.com/b\nVcs-Git: https://example.com/g\n",
            ),
            // XS- form on a Source tree is reported under its plain name.
            control(PKG_PERL, "XS-Vcs-Git: https://example.com/x.git\n"),
        ];
        SourceTreeTester::new(NoTeamUrl, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let bare = Vec::<(&str, TestFile)>::new();
        let pass = vec![
            SourcePackageCase::from(bare.clone())
                .dsc(dsc(PKG_PERL, &format!("Vcs-Git: {TEAM_GIT}\n"))),
            SourcePackageCase::from(bare.clone()).dsc(dsc(
                "Foo <foo@example.com>",
                "Vcs-Git: https://example.com/x.git\n",
            )),
        ];
        let fail = vec![
            SourcePackageCase::from(bare.clone())
                .dsc(dsc(PKG_PERL, "Vcs-Git: https://example.com/x.git\n")),
            SourcePackageCase::from(bare).dsc(dsc(PKG_PERL, "Vcs-Svn: svn://x\n")),
        ];
        SourcePackageTester::new(NoTeamUrl, pass, fail).test_and_snapshot();
    }
}
