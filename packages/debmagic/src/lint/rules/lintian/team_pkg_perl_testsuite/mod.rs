//! Rules for lintian's `team/pkg-perl/testsuite` Check.

mod ln1786_no_team_tests;

pub(crate) use ln1786_no_team_tests::NoTeamTests;
mod ln1782_no_testsuite_header;
mod ln1783_autopkgtest_needs_use_name;

pub(crate) use ln1782_no_testsuite_header::NoTestsuiteHeader;
pub(crate) use ln1783_autopkgtest_needs_use_name::AutopkgtestNeedsUseName;

use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;

/// The source paragraph lintian's `processable->fields` reads: the `.dsc` of a
/// Source package, else the source paragraph of `debian/control`.
fn source_fields<'a>(ctx: &'a SourceContext<'_>) -> Option<&'a ControlParagraph> {
    match ctx.dsc() {
        Some(dsc) => Some(dsc.fields()),
        None => ctx.debian_control().map(|control| control.source()),
    }
}

/// The Maintainer contains the pkg-perl team address. Lintian matches it as a
/// substring, so a name before or after it still counts.
fn is_pkg_perl(fields: &ControlParagraph) -> bool {
    fields.get("Maintainer").is_some_and(|maintainer| {
        maintainer.contains("pkg-perl-maintainers@lists.alioth.debian.org")
    })
}

/// `Testsuite` names `autopkgtest-pkg-perl`, split on commas.
fn uses_pkg_perl_testsuite(fields: &ControlParagraph) -> bool {
    fields.unfolded_value("Testsuite").is_some_and(|value| {
        value
            .split(',')
            .any(|name| name.trim() == "autopkgtest-pkg-perl")
    })
}
