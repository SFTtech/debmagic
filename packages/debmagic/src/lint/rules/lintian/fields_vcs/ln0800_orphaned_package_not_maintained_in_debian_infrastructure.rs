use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::{ControlParagraph, unfold};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::super::vcs::PLATFORMS;
use super::ln0799_orphaned_package_maintained_in_private_space::orphaned;

pub struct OrphanedPackageNotMaintainedInDebianInfrastructure;

declare_rule! {
    /// An orphaned package's `Vcs-*` repository is outside `*.debian.org`.
    ///
    /// Only for a Maintainer of `packages@qa.debian.org`. Reads `Vcs-Arch`,
    /// `-Bzr`, `-Cvs`, `-Darcs`, `-Hg`, `-Git`, `-Svn`, and `-Mtn` (not
    /// `Vcs-Browser`) from the Dsc, or the source paragraph when there is no
    /// Dsc. A non-empty URI without a `*.debian.org` host is flagged. Extra is
    /// the field name (for example `Vcs-Git`) and the unfolded URI. Location is
    /// empty.
    OrphanedPackageNotMaintainedInDebianInfrastructure,
    code = "LN0800",
    tag = "orphaned-package-not-maintained-in-debian-infrastructure",
    default_selected = true,
    experimental = false,
}

static DEBIAN_ORG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"//.+\.debian\.org/").expect("valid regex"));

impl SourceRule for OrphanedPackageNotMaintainedInDebianInfrastructure {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = match ctx.dsc() {
            Some(dsc) => messages(dsc.fields()),
            None => ctx
                .debian_control()
                .map(|control| messages(control.source()))
                .unwrap_or_default(),
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

fn messages(fields: &ControlParagraph) -> Vec<String> {
    if !orphaned(fields) {
        return Vec::new();
    }
    let mut found = Vec::new();
    for platform in PLATFORMS {
        if *platform == "Browser" {
            continue;
        }
        let name = format!("Vcs-{platform}");
        let Some(uri) = fields.get(&name).map(unfold) else {
            continue;
        };
        if !uri.is_empty() && !DEBIAN_ORG.is_match(&uri) {
            found.push(format!("{name} {uri}"));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(maintainer: &str, fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: {maintainer}\n\
                 Standards-Version: 4.7.2\n{fields}\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        const QA: &str = "Debian QA Group <packages@qa.debian.org>";
        const JANE: &str = "Jane <jane@example.com>";
        let pass = vec![
            control(JANE, "Vcs-Git: https://github.com/user/project\n"),
            control(QA, "Vcs-Git: https://salsa.debian.org/debian/foo.git\n"),
            control(QA, "Vcs-Git: https://git.dgit.debian.org/foo\n"),
            control(QA, "Vcs-Browser: https://github.com/user/project\n"),
            control(QA, "Vcs-Git:\n"),
        ];
        let fail = vec![
            control(QA, "Vcs-Git: https://github.com/user/project\n"),
            control(QA, "Vcs-Bzr: lp:foo\n"),
            control(
                QA,
                "Vcs-Git: https://salsa.debian.org/foo/bar.git\nVcs-Svn: https://example.com/svn\n",
            ),
        ];
        SourceTreeTester::new(
            OrphanedPackageNotMaintainedInDebianInfrastructure,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
