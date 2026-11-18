use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::{ControlParagraph, unfold};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::super::vcs::PLATFORMS;

pub struct OrphanedPackageMaintainedInPrivateSpace;

declare_rule! {
    /// An orphaned package keeps its `Vcs-*` repository in private space.
    ///
    /// Only for a Maintainer of `packages@qa.debian.org`. Reads `Vcs-Arch`,
    /// `-Bzr`, `-Cvs`, `-Darcs`, `-Hg`, `-Git`, `-Svn`, and `-Mtn` (not
    /// `Vcs-Browser`) from the Dsc, or the source paragraph when there is no
    /// Dsc. A URI on a `*.debian.org` host is flagged unless it is under
    /// `salsa.debian.org/debian/` or `git.dgit.debian.org/`. Extra is the
    /// field name (for example `Vcs-Git`) and the unfolded URI. Location is
    /// empty.
    OrphanedPackageMaintainedInPrivateSpace,
    code = "LN0799",
    tag = "orphaned-package-maintained-in-private-space",
    default_selected = true,
    experimental = false,
}

static INFRASTRUCTURE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"//.+\.debian\.org/").expect("valid regex"));
static PUBLIC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"//salsa\.debian\.org/debian/|//git\.dgit\.debian\.org/").expect("valid regex")
});

impl SourceRule for OrphanedPackageMaintainedInPrivateSpace {
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

/// Whether the Maintainer is the Debian QA group, that is, an orphaned package.
pub(super) fn orphaned(fields: &ControlParagraph) -> bool {
    fields
        .get("Maintainer")
        .is_some_and(|maintainer| maintainer.contains("packages@qa.debian.org"))
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
        if INFRASTRUCTURE.is_match(&uri) && !PUBLIC.is_match(&uri) {
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
            control(JANE, "Vcs-Git: https://salsa.debian.org/foo/bar.git\n"),
            control(QA, "Vcs-Git: https://salsa.debian.org/debian/foo.git\n"),
            control(QA, "Vcs-Git: https://git.dgit.debian.org/foo\n"),
            control(QA, "Vcs-Browser: https://salsa.debian.org/foo/bar\n"),
            control(QA, "Vcs-Git: https://github.com/foo/bar.git\n"),
            control(JANE, "Vcs-Svn: svn://svn.debian.org/foo/trunk\n"),
        ];
        let fail = vec![
            control(QA, "Vcs-Git: https://salsa.debian.org/lechner/pius.git\n"),
            control(QA, "Vcs-Git: https://anonscm.debian.org/git/foo/bar.git\n"),
            control(QA, "Vcs-Svn: svn://svn.debian.org/svn/foo/trunk\n"),
        ];
        SourceTreeTester::new(OrphanedPackageMaintainedInPrivateSpace, pass, fail)
            .test_and_snapshot();
    }
}
