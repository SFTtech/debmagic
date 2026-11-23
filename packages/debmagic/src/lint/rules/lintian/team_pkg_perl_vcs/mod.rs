//! Rules for lintian's `team/pkg-perl/vcs` Check.

mod ln1784_no_git;
mod ln1785_no_team_url;

pub(crate) use ln1784_no_git::NoGit;
pub(crate) use ln1785_no_team_url::NoTeamUrl;

/// The `Vcs-*` fields that name a non-git repository, in lintian's order.
const NON_GIT_VCS_FIELDS: [&str; 7] = [
    "Vcs-Arch",
    "Vcs-Bzr",
    "Vcs-Cvs",
    "Vcs-Darcs",
    "Vcs-Hg",
    "Vcs-Mtn",
    "Vcs-Svn",
];

/// Every `Vcs-*` field the Check looks at for a team location, in lintian's
/// order: the non-git ones, then `Vcs-Git` and `Vcs-Browser`.
const VCS_FIELDS: [&str; 9] = [
    "Vcs-Arch",
    "Vcs-Bzr",
    "Vcs-Cvs",
    "Vcs-Darcs",
    "Vcs-Hg",
    "Vcs-Mtn",
    "Vcs-Svn",
    "Vcs-Git",
    "Vcs-Browser",
];
