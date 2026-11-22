//! Rules for lintian's `shell/non-posix/bash-centric` Check.
//!
//! `scan` holds the line walk and the pattern table both Rules share.

mod ln1430_bash_term_in_posix_shell;
mod scan;

pub(crate) use ln1430_bash_term_in_posix_shell::BashTermInPosixShell;
