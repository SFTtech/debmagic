//! Rules for lintian's `template/dh-make/control/vcs` Check.
//!
//! Each Rule owns one Tag. The Check reads the packaging `debian/control`.

mod ln0846_control_file_contains_dh_make_vcs_comment;

pub(crate) use ln0846_control_file_contains_dh_make_vcs_comment::ControlFileContainsDhMakeVcsComment;
