//! Rules for lintian's `languages/javascript/nodejs` Check.
//!
//! Each Rule owns one Tag. Installed paths are matched on their own.
//! Directories are ignored, and packages whose names end in `-dbg` are not
//! checked.

mod detect;
mod ln0403_pkg_js_tools_test_is_empty;
mod ln0404_pkg_js_tools_test_is_missing;
mod ln0434_nodejs_module_installed_in_usr_lib;
mod ln0446_node_package_install_in_nodejs_rootdir;
mod ln0447_nodejs_lock_file;

pub(crate) use ln0403_pkg_js_tools_test_is_empty::PkgJsToolsTestIsEmpty;
pub(crate) use ln0404_pkg_js_tools_test_is_missing::PkgJsToolsTestIsMissing;
pub(crate) use ln0434_nodejs_module_installed_in_usr_lib::NodejsModuleInstalledInUsrLib;
pub(crate) use ln0446_node_package_install_in_nodejs_rootdir::NodePackageInstallInNodejsRootdir;
pub(crate) use ln0447_nodejs_lock_file::NodejsLockFile;
