use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::super::vcs::PLATFORMS;

pub struct VcsFieldsUseMoreThanOneVcs;

declare_rule! {
    /// The `Vcs-*` fields name more than one version control system.
    ///
    /// Every declared `Vcs-*` field counts, except `Vcs-Browser`, whatever
    /// its value. Reads the Dsc, or the source paragraph of `debian/control`
    /// when there is no Dsc. Extra is the sorted, lowercased platform names
    /// separated by spaces, for example `bzr git`. Location is empty.
    /// Severity is info and the Tag is not selected by default, as in lintian.
    VcsFieldsUseMoreThanOneVcs,
    code = "LN1373",
    tag = "vcs-fields-use-more-than-one-vcs",
    default_selected = false,
    experimental = false,
}

impl SourceRule for VcsFieldsUseMoreThanOneVcs {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let found = match ctx.dsc() {
            Some(dsc) => seen_vcs(dsc.fields()),
            None => ctx
                .debian_control()
                .map(|control| seen_vcs(control.source()))
                .unwrap_or_default(),
        };
        if found.len() > 1 {
            ctx.diagnostic(Diagnostic::warn(found.join(" ")).with_severity(Severity::Info));
        }
    }
}

/// Lowercased, sorted names of the declared version control systems.
fn seen_vcs(fields: &ControlParagraph) -> Vec<String> {
    let mut found: Vec<String> = PLATFORMS
        .iter()
        .filter(|platform| **platform != "Browser")
        .filter(|platform| fields.declares(&format!("Vcs-{platform}")))
        .map(|platform| platform.to_lowercase())
        .collect();
    found.sort();
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
            control(
                "Vcs-Git: https://salsa.debian.org/foo/bar.git\n\
                 Vcs-Browser: https://salsa.debian.org/foo/bar\n",
            ),
        ];
        let fail = vec![
            control(
                "Vcs-Git: https://salsa.debian.org/foo/bar.git\n\
                 Vcs-Svn: svn+ssh://svn.debian.org/svn/foobar/trunk\n",
            ),
            control(
                "Vcs-Git: https://salsa.debian.org/foo/bar.git\n\
                 Vcs-Bzr: lp:foobar\n\
                 Vcs-Hg: https://hg.example.org/foobar\n",
            ),
        ];
        SourceTreeTester::new(VcsFieldsUseMoreThanOneVcs, pass, fail).test_and_snapshot();
    }
}
