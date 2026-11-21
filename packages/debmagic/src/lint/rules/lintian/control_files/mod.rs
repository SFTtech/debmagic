//! Rules for lintian's `control-files` Check.

mod ln1015_control_file_has_bad_owner;

pub(crate) use ln1015_control_file_has_bad_owner::ControlFileHasBadOwner;
