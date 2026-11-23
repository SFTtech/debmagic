use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::super::vcs::{PLATFORMS, vcs_field_value};

pub struct CoMaintainedPackageWithNoVcsFields;

declare_rule! {
    /// The package is maintained by a team or by several people, and no `Vcs-*`
    /// field names a repository.
    ///
    /// Lintian's `fields/vcs` Check on the source. Co-maintained means an
    /// `Uploaders` field with at least one uploader and no mailing list in
    /// `Maintainer` or `Uploaders`; a team means a mailing list
    /// (`lists.debian.org`, `lists.alioth.debian.org`, `alioth-lists.debian.net`,
    /// `tracker.debian.org` or `security.debian.org`) in either field. A
    /// `Vcs-*` field counts whatever its value, except `Vcs-Browser`. Reads the
    /// Dsc, or the source paragraph of `debian/control` when there is no Dsc,
    /// where an `XS-` style field counts under its plain name as on the Dsc.
    /// Extra is empty and the Location is empty. Severity is pedantic and the
    /// Tag is not selected by default, as in lintian.
    CoMaintainedPackageWithNoVcsFields,
    code = "LN1660",
    tag = "co-maintained-package-with-no-vcs-fields",
    default_selected = false,
    experimental = false,
}

static TEAM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b\S+@(?:lists(?:\.alioth)?\.debian\.org|alioth-lists\.debian\.net|tracker\.debian\.org|security\.debian\.org)\b",
    )
    .expect("valid regex")
});

impl SourceRule for CoMaintainedPackageWithNoVcsFields {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let source_tree = ctx.dsc().is_none();
        let flagged = {
            let fields: &ControlParagraph = match ctx.dsc() {
                Some(dsc) => dsc.fields(),
                None => match ctx.debian_control() {
                    Some(control) => control.source(),
                    None => return,
                },
            };
            co_maintained_without_vcs(fields, source_tree)
        };
        if flagged {
            ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Pedantic));
        }
    }
}

/// Lintian's `always`: a team or co-maintained package, and no `Vcs-*` field
/// other than `Vcs-Browser`.
fn co_maintained_without_vcs(fields: &ControlParagraph, source_tree: bool) -> bool {
    let mut team = false;
    let mut co_maintained = false;
    for name in ["Maintainer", "Uploaders"] {
        let Some(value) = fields.unfolded_value(name) else {
            continue;
        };
        if TEAM.is_match(&value) {
            team = true;
        }
        if name == "Uploaders" {
            // Lintian splits on commas: any non-blank piece is an uploader.
            let uploaders = value.split(',').any(|piece| !piece.trim().is_empty());
            if uploaders && !team {
                co_maintained = true;
            }
        }
    }
    if !(team || co_maintained) {
        return false;
    }
    // An empty `Vcs-*` field is not declared (the Parity recipe
    // `control-has-empty-field` expects the Tag for one).
    let has_vcs = PLATFORMS.iter().any(|platform| {
        *platform != "Browser"
            && vcs_field_value(fields, &format!("Vcs-{platform}"), source_tree)
                .is_some_and(|value| !value.trim().is_empty())
    });
    !has_vcs
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
    fn source() {
        const JANE: &str = "Jane <jane@example.com>";
        const TEAM: &str = "Example Team <team@lists.debian.org>";
        let pass = vec![
            control(JANE, ""),
            control(
                JANE,
                "Uploaders: Bob <bob@example.com>\nVcs-Git: https://salsa.debian.org/foo/bar.git\n",
            ),
            control(
                JANE,
                "Uploaders: Bob <bob@example.com>\nXS-Vcs-Git: https://salsa.debian.org/foo/bar.git\n",
            ),
            control(TEAM, "Vcs-Svn: svn://svn.example.com/trunk\n"),
            // An empty Uploaders field names no co-maintainer.
            control(JANE, "Uploaders:\n"),
        ];
        let fail = vec![
            control(JANE, "Uploaders: Bob <bob@example.com>\n"),
            control(TEAM, ""),
            control(
                JANE,
                "Uploaders: Bob <bob@example.com>\nVcs-Browser: https://salsa.debian.org/foo/bar\n",
            ),
        ];
        SourceTreeTester::new(CoMaintainedPackageWithNoVcsFields, pass, fail).test_and_snapshot();
    }
}
