//! Rules for lintian's `control-files` Check.

mod ln1015_control_file_has_bad_owner;
mod ln1179_control_file_has_bad_permissions;
mod ln1180_control_file_is_empty;
mod ln1181_control_file_is_not_a_file;
mod ln1182_not_allowed_control_file;
mod ln1183_unknown_control_file;

pub(crate) use ln1015_control_file_has_bad_owner::ControlFileHasBadOwner;
pub(crate) use ln1179_control_file_has_bad_permissions::ControlFileHasBadPermissions;
pub(crate) use ln1180_control_file_is_empty::ControlFileIsEmpty;
pub(crate) use ln1181_control_file_is_not_a_file::ControlFileIsNotAFile;
pub(crate) use ln1182_not_allowed_control_file::NotAllowedControlFile;
pub(crate) use ln1183_unknown_control_file::UnknownControlFile;
