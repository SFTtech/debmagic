use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct VcsObsoleteInDebianInfrastructure;

declare_rule! {
    /// A `Vcs-*` field points at a retired `*.debian.org` host.
    ///
    /// Checks `Vcs-Browser`, `-Arch`, `-Bzr`, `-Cvs`, `-Darcs`, `-Hg`, `-Git`,
    /// `-Svn`, and `-Mtn` of the source (the Dsc on a Source package, the
    /// source paragraph of `debian/control` on a Source tree). The unfolded
    /// value must contain `//<host>.debian.org/` where the greedy `<host>` is
    /// neither `salsa` nor ends in `.dgit`. Extra is the platform, then the
    /// whole unfolded value (branch and subdirectory included). The Location
    /// is empty.
    VcsObsoleteInDebianInfrastructure,
    code = "LN0023",
    tag = "vcs-obsolete-in-debian-infrastructure",
    default_selected = true,
    experimental = false,
}

/// Lintian's `%VCS_EXTRACT` keys.
const PLATFORMS: &[&str] = &[
    "Browser", "Arch", "Bzr", "Cvs", "Darcs", "Hg", "Git", "Svn", "Mtn",
];

static DEBIAN_ORG_HOST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"//(.+)\.debian\.org/").expect("valid regex"));
static CURRENT_HOST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:salsa|.*\.dgit)$").expect("valid regex"));
static FOLD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*").expect("valid regex"));

fn unfold(value: &str) -> String {
    FOLD.replace_all(value.trim(), " ").into_owned()
}

impl SourceRule for VcsObsoleteInDebianInfrastructure {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let values: Vec<(&str, String)> = {
            let get = |name: &str| match ctx.dsc() {
                Some(dsc) => dsc.fields().get(name).map(unfold),
                None => ctx
                    .debian_control()
                    .and_then(|control| control.source().get(name).map(unfold)),
            };
            PLATFORMS
                .iter()
                .filter_map(|platform| Some((*platform, get(&format!("Vcs-{platform}"))?)))
                .collect()
        };
        for (platform, uri) in values {
            if uri.is_empty() {
                continue;
            }
            let obsolete = DEBIAN_ORG_HOST
                .captures(&uri)
                .is_some_and(|captures| !CURRENT_HOST.is_match(&captures[1]));
            if obsolete {
                ctx.diagnostic(Diagnostic::warn(format!("{platform} {uri}")));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
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
            control("Vcs-Git: https://salsa.debian.org/foo/bar.git\n"),
            control("Vcs-Browser: https://salsa.debian.org/foo/bar\n"),
            control("Vcs-Git: https://github.com/foo/bar.git\n"),
            control("Vcs-Git: https://git.dgit.debian.org/bar\n"),
            control("Vcs-Git: https://example.com/debian.org/\n"),
            control("Vcs-Git: git@salsa.debian.org:foo/bar.git\n"),
            control("Vcs-Git:\n"),
        ];
        let fail = vec![
            control("Vcs-Svn: svn://anonscm.debian.org/foobar/trunk\n"),
            control("Vcs-Git: https://anonscm.debian.org/git/foo.git -b master\n"),
            control("Vcs-Browser: https://git.debian.org/?p=foo.git\n"),
            control("Vcs-Hg: http://hg.debian.org/hg/foo\n"),
            control("Vcs-Git: https://anonscm.debian.org/git/foo.git\n  [sub]\n"),
            control(
                "Vcs-Bzr: bzr://bzr.debian.org/foo\nVcs-Cvs: :pserver:anon@cvs.debian.org/cvs\n",
            ),
            control("Vcs-Git: https://alioth.debian.org/x\n"),
            control("Vcs-Git: https://a.debian.org/salsa.debian.org/foo\n"),
        ];
        SourceTreeTester::new(VcsObsoleteInDebianInfrastructure, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let dsc = |extra: &str| Dsc::parse(&format!("Source: example\n{extra}")).expect("Dsc");
        let pass = vec![
            SourcePackageCase::from(control(""))
                .dsc(dsc("Vcs-Git: https://salsa.debian.org/a/b\n")),
            // The Patched control is not the oracle once a Dsc exists.
            SourcePackageCase::from(control("Vcs-Svn: svn://anonscm.debian.org/x\n")).dsc(dsc("")),
        ];
        let fail = vec![
            SourcePackageCase::from(control(""))
                .dsc(dsc("Vcs-Svn: svn://anonscm.debian.org/foobar/trunk\n")),
        ];
        SourcePackageTester::new(VcsObsoleteInDebianInfrastructure, pass, fail).test_and_snapshot();
    }
}
