//! Rules for lintian's `shell/non-posix/bash-centric` Check.
//!
//! `scan` holds the line walk and the pattern table both Rules share.

mod ln1430_bash_term_in_posix_shell;
mod ln1431_possible_bashism_in_maintainer_script;
mod scan;

pub(crate) use ln1430_bash_term_in_posix_shell::BashTermInPosixShell;
pub(crate) use ln1431_possible_bashism_in_maintainer_script::PossibleBashismInMaintainerScript;
