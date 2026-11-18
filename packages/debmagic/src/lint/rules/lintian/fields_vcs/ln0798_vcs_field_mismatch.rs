use std::sync::LazyLock;

use regex::{Regex, RegexBuilder};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::data::lines;
use crate::lint::debian_control::{ControlParagraph, unfold};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::super::vcs::PLATFORMS;

pub struct VcsFieldMismatch;

declare_rule! {
    /// A `Vcs-*` URI names a hosting service that serves another VCS.
    ///
    /// Reads `Vcs-Arch`, `-Bzr`, `-Cvs`, `-Darcs`, `-Hg`, `-Git`, `-Svn`, and
    /// `-Mtn` from the Dsc, or the source paragraph when there is no Dsc
    /// (`Vcs-Browser` is not checked). A URI that starts with a hosting pattern
    /// of the vendored `fields/vcs-hosters` table, ignoring case, must be under
    /// the field for the VCS that service serves (its first listed VCS). Extra
    /// is `Vcs-<platform> != Vcs-<known> <uri>`. Location is empty.
    VcsFieldMismatch,
    code = "LN0798",
    tag = "vcs-field-mismatch",
    default_selected = true,
    experimental = false,
}

/// Hosting pattern (anchored, case-insensitive) with the VCS it serves.
static HOSTERS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    lines(include_str!("../../../data/fields/vcs-hosters"))
        .filter_map(|line| {
            let (pattern, vcses) = line.split_once("~~")?;
            let known = vcses.split(',').next()?.trim();
            let regex = RegexBuilder::new(&format!("^(?:{})", pattern.trim()))
                .case_insensitive(true)
                .build()
                .expect("valid regex");
            Some((regex, known))
        })
        .collect()
});

impl SourceRule for VcsFieldMismatch {
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
    let mut found = Vec::new();
    for platform in PLATFORMS {
        if *platform == "Browser" {
            continue;
        }
        let name = format!("Vcs-{platform}");
        let Some(uri) = fields.get(&name).map(unfold) else {
            continue;
        };
        if uri.is_empty() {
            continue;
        }
        for (regex, known) in HOSTERS.iter() {
            if regex.is_match(&uri) && *known != *platform {
                let message = format!("Vcs-{platform} != Vcs-{known} {uri}");
                if !found.contains(&message) {
                    found.push(message);
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Jane <jane@example.com>\n\
                 Standards-Version: 4.7.2\n{fields}\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            control("Vcs-Git: https://salsa.debian.org/foo/bar.git\n"),
            control("Vcs-Browser: https://github.com/foo/bar\n"),
            control("Vcs-Svn: svn://anonscm.debian.org/foo/trunk\n"),
            control("Vcs-Bzr: lp:foo\n"),
            control("Vcs-Git: https://example.com/foo.git\n"),
        ];
        let fail = vec![
            control("Vcs-Svn: https://salsa.debian.org/lintian/lintian.git\n"),
            control("Vcs-Bzr: https://gitlab.org/lintian/lintian.git\n"),
            control("Vcs-Hg: HTTPS://GitHub.com/foo/bar\n"),
            control("Vcs-Svn: https://github.com/foo/bar\nVcs-Hg: https://github.com/x/y\n"),
        ];
        SourceTreeTester::new(VcsFieldMismatch, pass, fail).test_and_snapshot();
    }
}
