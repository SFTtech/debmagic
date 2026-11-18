//! Maintainer-script shell Tags from lintian's `scripts` Check.
//!
//! Each Rule owns one Tag. The walk of Control files lives in `scan`.
//! Interpreter-table Tags are not implemented here.

mod ln0158_maintainer_script_ignores_errors;
mod ln0159_read_in_maintainer_script;
mod ln0160_recursive_privilege_change;
mod ln0161_command_with_path_in_maintainer_script;
mod ln0162_maintainer_script_removes_device_files;
mod scan;

pub(crate) use ln0158_maintainer_script_ignores_errors::MaintainerScriptIgnoresErrors;
pub(crate) use ln0159_read_in_maintainer_script::ReadInMaintainerScript;
pub(crate) use ln0160_recursive_privilege_change::RecursivePrivilegeChange;
pub(crate) use ln0161_command_with_path_in_maintainer_script::CommandWithPathInMaintainerScript;
pub(crate) use ln0162_maintainer_script_removes_device_files::MaintainerScriptRemovesDeviceFiles;
