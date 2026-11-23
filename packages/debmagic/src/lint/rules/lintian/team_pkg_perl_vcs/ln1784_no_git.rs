use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::super::team_pkg_perl::{is_pkg_perl, source_fields, vcs_field};
use super::NON_GIT_VCS_FIELDS;

pub struct NoGit;

declare_rule! {
    /// A pkg-perl package (its Maintainer names `pkg-perl-maintainers@lists.alioth.debian.org`)
    /// still declares a `Vcs-*` field for a non-git repository. One error per
    /// such field, with the field's name as the extra, in the order Arch, Bzr,
    /// Cvs, Darcs, Hg, Mtn, Svn.
    ///
    /// Lintian's `team/pkg-perl/vcs` Check. Reads the Dsc, or the source
    /// paragraph of `debian/control` on a Source tree. A field counts when it is
    /// declared, whatever its value; on a Source tree an `XS-` field counts under
    /// its plain name. Location is empty.
    NoGit,
    code = "LN1784",
    tag = "no-git",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NoGit {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let source_tree = ctx.dsc().is_none();
        let declared: Vec<&str> = {
            let Some(fields) = source_fields(ctx) else {
                return;
            };
            if !is_pkg_perl(fields) {
                return;
            }
            NON_GIT_VCS_FIELDS
                .into_iter()
                .filter(|name| vcs_field(fields, name, source_tree).is_some())
                .collect()
        };
        for name in declared {
            ctx.diagnostic(Diagnostic::error(name));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const PKG_PERL: &str = "Debian Perl Group <pkg-perl-maintainers@lists.alioth.debian.org>";

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
                "Vcs-Git: https://salsa.debian.org/perl-team/modules/packages/libfoo-perl.git\n",
            ),
            control(PKG_PERL, ""),
            // Not a pkg-perl package: the non-git field is fine.
            control(
                "Foo <foo@example.com>",
                "Vcs-Svn: svn://example.com/trunk\n",
            ),
            // The mailing list must be in the Maintainer, not elsewhere.
            control(
                "Foo <foo@example.com>",
                "Uploaders: Debian Perl Group <pkg-perl-maintainers@lists.alioth.debian.org>\nVcs-Svn: svn://example.com/trunk\n",
            ),
            // The list is matched case-sensitively.
            control(
                "Foo <PKG-PERL-MAINTAINERS@lists.alioth.debian.org>",
                "Vcs-Svn: svn://x\n",
            ),
            control(PKG_PERL, "XS-Vcs-Git: https://salsa.debian.org/x.git\n"),
            Vec::<(&str, TestFile)>::new(),
            // A non-UTF-8 debian/control is not read at all.
            vec![(
                "debian/control",
                TestFile::from_bytes(
                    b"Source: libfoo-perl\nMaintainer: \xff\nVcs-Svn: x\n".to_vec(),
                ),
            )],
            // debian/control as a symlink is not read either.
            vec![("debian/control", TestFile::symlink("/nonexistent"))],
        ];
        let fail = vec![
            control(PKG_PERL, "Vcs-Svn: svn://example.com/trunk\n"),
            control(PKG_PERL, "vcs-svn: svn://example.com/trunk\n"),
            // Several fields: one hint each, in the Check's order.
            control(
                PKG_PERL,
                "Vcs-Svn: svn://x\nVcs-Arch: arch://x\nVcs-Hg: hg://x\n",
            ),
            // An empty value still declares the field.
            control(PKG_PERL, "Vcs-Cvs:\n"),
            // XS- form on a Source tree counts under its plain name.
            control(PKG_PERL, "XS-Vcs-Bzr: bzr://x\n"),
        ];
        SourceTreeTester::new(NoGit, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let bare = Vec::<(&str, TestFile)>::new();
        let pass = vec![
            SourcePackageCase::from(bare.clone())
                .dsc(dsc(PKG_PERL, "Vcs-Git: https://salsa.debian.org/x.git\n")),
            SourcePackageCase::from(bare.clone())
                .dsc(dsc("Foo <foo@example.com>", "Vcs-Svn: svn://x\n")),
            // A Dsc has no XS- form: the prefixed field is not a Vcs-Svn.
            SourcePackageCase::from(bare.clone()).dsc(dsc(PKG_PERL, "XS-Vcs-Svn: svn://x\n")),
        ];
        let fail = vec![
            SourcePackageCase::from(bare.clone()).dsc(dsc(PKG_PERL, "Vcs-Svn: svn://x\n")),
            SourcePackageCase::from(bare)
                .dsc(dsc(PKG_PERL, "Vcs-Darcs: darcs://x\nVcs-Mtn: mtn://x\n")),
        ];
        SourcePackageTester::new(NoGit, pass, fail).test_and_snapshot();
    }
}
