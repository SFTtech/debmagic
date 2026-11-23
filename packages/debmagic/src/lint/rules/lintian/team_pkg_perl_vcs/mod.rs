//! Rules for lintian's `team/pkg-perl/vcs` Check.

mod ln1784_no_git;

pub(crate) use ln1784_no_git::NoGit;

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
