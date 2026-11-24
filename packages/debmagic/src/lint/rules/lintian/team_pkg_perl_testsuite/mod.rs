//! Rules for lintian's `team/pkg-perl/testsuite` Check.

mod ln1786_no_team_tests;

pub(crate) use ln1786_no_team_tests::NoTeamTests;
mod ln1782_no_testsuite_header;
mod ln1783_autopkgtest_needs_use_name;

pub(crate) use ln1782_no_testsuite_header::NoTestsuiteHeader;
pub(crate) use ln1783_autopkgtest_needs_use_name::AutopkgtestNeedsUseName;

use super::team_pkg_perl::{is_pkg_perl, source_fields};
use crate::lint::debian_control::ControlParagraph;

/// `Testsuite` names `autopkgtest-pkg-perl`, split on commas.
fn uses_pkg_perl_testsuite(fields: &ControlParagraph) -> bool {
    fields.unfolded_value("Testsuite").is_some_and(|value| {
        value
            .split(',')
            .any(|name| name.trim() == "autopkgtest-pkg-perl")
    })
}
