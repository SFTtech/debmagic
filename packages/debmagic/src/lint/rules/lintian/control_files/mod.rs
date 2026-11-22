//! Rules for lintian's `control-files` Check.

mod ln1015_control_file_has_bad_owner;
mod ln1179_control_file_has_bad_permissions;
mod ln1180_control_file_is_empty;

pub(crate) use ln1015_control_file_has_bad_owner::ControlFileHasBadOwner;
pub(crate) use ln1179_control_file_has_bad_permissions::ControlFileHasBadPermissions;
pub(crate) use ln1180_control_file_is_empty::ControlFileIsEmpty;
